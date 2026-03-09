//! SQL migration file parser.
//!
//! Parses DDL statements from SQL migration files into core `Schema` objects.
//! Supports both Postgres and MySQL dialects.

use std::path::{Path, PathBuf};

use crate::config::Engine;
use crate::error::ParseError;
use crate::schema::*;

/// Parse all migration files in a directory and build a cumulative schema.
///
/// Files are sorted lexicographically (matching numbered migration conventions)
/// and applied sequentially.
pub fn parse_migrations(dir: &Path, dialect: Engine) -> Result<Schema, ParseError> {
    if !dir.is_dir() {
        return Err(ParseError::DirectoryNotFound(dir.to_path_buf()));
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| ParseError::ReadFile {
            path: dir.to_path_buf(),
            source: e,
        })?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sql"))
        .collect();

    files.sort();

    if files.is_empty() {
        return Err(ParseError::NoMigrations(dir.to_path_buf()));
    }

    let default_schema = match dialect {
        Engine::Postgres => "public",
        Engine::Mysql => "default",
    };
    let mut schema = Schema::new(default_schema);

    for file in &files {
        let sql = std::fs::read_to_string(file).map_err(|e| ParseError::ReadFile {
            path: file.clone(),
            source: e,
        })?;
        parse_sql(&sql, dialect, file, &mut schema)?;
    }

    Ok(schema)
}

/// Parse a single SQL string and apply DDL statements to the schema.
pub fn parse_sql(
    sql: &str,
    dialect: Engine,
    file: &Path,
    schema: &mut Schema,
) -> Result<(), ParseError> {
    let statements = split_statements(sql);

    for (line_num, stmt) in statements {
        let normalized = normalize_statement(&stmt);
        let upper = normalized.to_uppercase();

        if upper.starts_with("CREATE TABLE") {
            parse_create_table(&normalized, dialect, file, line_num, schema)?;
        } else if upper.starts_with("CREATE TYPE") {
            parse_create_type(&normalized, file, line_num, schema)?;
        } else if upper.starts_with("CREATE UNIQUE INDEX") || upper.starts_with("CREATE INDEX") {
            parse_create_index(&normalized, file, line_num, schema)?;
        } else if upper.starts_with("ALTER TABLE") {
            parse_alter_table(&normalized, dialect, file, line_num, schema)?;
        } else if upper.starts_with("DROP TABLE") {
            parse_drop_table(&normalized, schema);
        } else if upper.starts_with("DROP INDEX") {
            parse_drop_index(&normalized, schema);
        }
        // Silently skip other statements (INSERT, COMMENT, SET, etc.)
    }

    Ok(())
}

/// Split SQL text into individual statements, tracking line numbers.
fn split_statements(sql: &str) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;
    let mut in_string = false;
    let mut string_char = '"';
    let mut in_dollar_quote = false;
    let mut dollar_tag = String::new();
    let mut line_num = 1;

    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '\n' {
            line_num += 1;
        }

        if in_dollar_quote {
            current.push(ch);
            // Check for closing dollar quote
            if ch == '$' {
                let remaining: String = chars[i..].iter().collect();
                let tag = format!("${}$", dollar_tag);
                if remaining.starts_with(&tag) {
                    for tc in tag.chars().skip(1) {
                        current.push(tc);
                        i += 1;
                        if tc == '\n' {
                            line_num += 1;
                        }
                    }
                    in_dollar_quote = false;
                }
            }
            i += 1;
            continue;
        }

        if in_string {
            current.push(ch);
            if ch == string_char {
                // Check for escaped quote ('' or "")
                if i + 1 < chars.len() && chars[i + 1] == string_char {
                    current.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Start of string
        if ch == '\'' || ch == '"' {
            in_string = true;
            string_char = ch;
            current.push(ch);
            i += 1;
            continue;
        }

        // Dollar quoting (Postgres)
        if ch == '$' {
            let remaining: String = chars[i..].iter().collect();
            if let Some(end) = remaining[1..].find('$') {
                let tag = &remaining[1..end + 1];
                if tag.is_empty() || tag.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    dollar_tag = tag.to_string();
                    in_dollar_quote = true;
                    let full_tag = format!("${}$", tag);
                    current.push_str(&full_tag);
                    i += full_tag.len();
                    continue;
                }
            }
            current.push(ch);
            i += 1;
            continue;
        }

        // Single-line comment
        if ch == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Multi-line comment
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line_num += 1;
                }
                i += 1;
            }
            i += 2; // skip */
            continue;
        }

        if ch == ';' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                results.push((start_line, trimmed));
            }
            current.clear();
            start_line = line_num;
            i += 1;
            continue;
        }

        if current.is_empty() && ch.is_whitespace() {
            if ch == '\n' {
                start_line = line_num;
            }
            i += 1;
            continue;
        }

        current.push(ch);
        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        results.push((start_line, trimmed));
    }

    results
}

fn normalize_statement(s: &str) -> String {
    // Collapse whitespace but preserve quoted strings
    let mut result = String::with_capacity(s.len());
    let mut in_string = false;
    let mut string_char = '"';
    let mut last_was_space = false;

    for ch in s.chars() {
        if in_string {
            result.push(ch);
            if ch == string_char {
                in_string = false;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            in_string = true;
            string_char = ch;
            result.push(ch);
            last_was_space = false;
            continue;
        }
        if ch.is_whitespace() {
            if !last_was_space && !result.is_empty() {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }

    result.trim().to_string()
}

fn parse_create_table(
    stmt: &str,
    dialect: Engine,
    file: &Path,
    line: usize,
    schema: &mut Schema,
) -> Result<(), ParseError> {
    // Extract table name - handle IF NOT EXISTS and schema-qualified names
    let upper = stmt.to_uppercase();
    let after_table = if let Some(pos) = upper.find("CREATE TABLE") {
        &stmt[pos + 12..]
    } else {
        return Ok(());
    };
    let after_table = after_table.trim();
    let after_table = if after_table.to_uppercase().starts_with("IF NOT EXISTS") {
        after_table[13..].trim()
    } else {
        after_table
    };

    // Find the opening parenthesis
    let paren_pos = match after_table.find('(') {
        Some(p) => p,
        None => return Ok(()),
    };

    let table_name_raw = after_table[..paren_pos].trim();
    let table_name = unquote(table_name_raw);
    // Handle schema-qualified: schema.table
    let table_name = if let Some(dot) = table_name.rfind('.') {
        table_name[dot + 1..].to_string()
    } else {
        table_name
    };

    // Find matching closing paren (accounting for nested parens)
    let body_start = paren_pos + 1;
    let body = &after_table[body_start..];
    let body_end = find_matching_paren(body).unwrap_or(body.len());
    let body = &body[..body_end];

    let mut table = Table::new(&table_name);

    // Split body by commas (respecting parentheses nesting)
    let parts = split_by_comma(body);

    for part in &parts {
        let trimmed = part.trim();
        let upper_part = trimmed.to_uppercase();

        if upper_part.starts_with("PRIMARY KEY") {
            if let Some(cols) = extract_paren_list(trimmed) {
                table.primary_key = Some(PrimaryKey {
                    name: None,
                    columns: cols,
                });
            }
        } else if upper_part.starts_with("UNIQUE") {
            if let Some(cols) = extract_paren_list(trimmed) {
                // Extract constraint name if present
                let name = extract_constraint_name(trimmed);
                table.unique_constraints.push(UniqueConstraint {
                    name,
                    columns: cols,
                });
            }
        } else if upper_part.starts_with("FOREIGN KEY") || upper_part.starts_with("CONSTRAINT") {
            if let Some(fk) = parse_inline_fk(trimmed) {
                table.foreign_keys.push(fk);
            }
        } else if upper_part.starts_with("CHECK") {
            if let Some(cc) = parse_inline_check(trimmed) {
                table.check_constraints.push(cc);
            }
        } else {
            // Column definition
            if let Some(col) = parse_column_def(trimmed, dialect, file, line)? {
                // Check for inline PRIMARY KEY
                if upper_part.contains(" PRIMARY KEY") && table.primary_key.is_none() {
                    table.primary_key = Some(PrimaryKey {
                        name: None,
                        columns: vec![col.name.clone()],
                    });
                }
                // Check for inline UNIQUE
                if upper_part.contains(" UNIQUE") {
                    table.unique_constraints.push(UniqueConstraint {
                        name: None,
                        columns: vec![col.name.clone()],
                    });
                }
                // Check for inline REFERENCES (FK)
                if upper_part.contains("REFERENCES") {
                    if let Some(fk) = parse_column_reference(trimmed, &col.name) {
                        table.foreign_keys.push(fk);
                    }
                }
                table.columns.push(col);
            }
        }
    }

    // Parse table options after closing paren (MySQL ENGINE, CHARSET, etc.)
    let after_body = &after_table[body_start + body_end..];
    if let Some(rest) = after_body.strip_prefix(')') {
        parse_table_options(rest, &mut table.options);
    }

    // Replace existing table or add new
    if let Some(existing) = schema.tables.iter_mut().find(|t| t.name == table_name) {
        *existing = table;
    } else {
        schema.tables.push(table);
    }

    Ok(())
}

fn parse_column_def(
    part: &str,
    dialect: Engine,
    _file: &Path,
    _line: usize,
) -> Result<Option<Column>, ParseError> {
    let tokens = tokenize_column_def(part);
    if tokens.len() < 2 {
        return Ok(None);
    }

    let name = unquote(&tokens[0]);

    // Skip if this looks like a constraint keyword, not a column name
    let upper_name = name.to_uppercase();
    if [
        "PRIMARY",
        "FOREIGN",
        "UNIQUE",
        "CHECK",
        "CONSTRAINT",
        "INDEX",
        "KEY",
    ]
    .contains(&upper_name.as_str())
    {
        return Ok(None);
    }

    let type_str = &tokens[1];
    let data_type = parse_data_type(type_str, &tokens[2..], dialect);

    let rest = part.to_uppercase();
    let nullable = !rest.contains("NOT NULL");
    let auto_increment = rest.contains("AUTO_INCREMENT")
        || rest.contains("SERIAL")
        || rest.contains("GENERATED")
        || rest.contains("AUTOINCREMENT");
    let unsigned = rest.contains("UNSIGNED");

    let default = extract_default(part);

    Ok(Some(Column {
        name,
        data_type,
        nullable,
        default,
        auto_increment,
        unsigned,
    }))
}

fn parse_data_type(type_str: &str, rest: &[String], dialect: Engine) -> DataType {
    let upper = type_str.to_uppercase();
    let upper = upper.trim();

    // Handle types with parenthesized parameters
    // Use original type_str for params to preserve case (important for ENUM values)
    let (base, params, original_params) = if let Some(paren_pos) = upper.find('(') {
        let base = &upper[..paren_pos];
        let params_str = &upper[paren_pos + 1..upper.len() - upper.ends_with(')') as usize];
        let params_str = params_str.trim_end_matches(')');
        let orig_params_str = &type_str.trim()
            [paren_pos + 1..type_str.trim().len() - type_str.trim().ends_with(')') as usize];
        let orig_params_str = orig_params_str.trim_end_matches(')');
        (
            base.to_string(),
            Some(params_str.to_string()),
            Some(orig_params_str.to_string()),
        )
    } else {
        (upper.to_string(), None, None)
    };

    match base.as_str() {
        "SMALLINT" | "INT2" | "SMALLSERIAL" => DataType::SmallInt,
        "INTEGER" | "INT" | "INT4" | "SERIAL" => DataType::Integer,
        "BIGINT" | "INT8" | "BIGSERIAL" => DataType::BigInt,
        "TINYINT" => {
            if dialect == Engine::Mysql {
                DataType::TinyInt
            } else {
                DataType::SmallInt
            }
        }
        "MEDIUMINT" => DataType::MediumInt,
        "REAL" | "FLOAT4" | "FLOAT" => DataType::Real,
        "DOUBLE" | "FLOAT8" => DataType::DoublePrecision,
        "NUMERIC" | "DECIMAL" => {
            let (p, s) = parse_precision_scale(&params);
            DataType::Numeric {
                precision: p,
                scale: s,
            }
        }
        "VARCHAR" | "CHARACTER VARYING" => {
            let len = params.as_ref().and_then(|p| p.trim().parse().ok());
            DataType::Varchar(len)
        }
        "CHAR" | "CHARACTER" => {
            let len = params.as_ref().and_then(|p| p.trim().parse().ok());
            DataType::Char(len)
        }
        "TEXT" => DataType::Text,
        "MEDIUMTEXT" => DataType::MediumText,
        "LONGTEXT" => DataType::LongText,
        "BOOLEAN" | "BOOL" => DataType::Boolean,
        "DATE" => DataType::Date,
        "TIME" => DataType::Time,
        "TIMESTAMP" | "TIMESTAMPTZ" => {
            let with_tz = upper.contains("TIMESTAMPTZ")
                || rest.iter().any(|t| {
                    let u = t.to_uppercase();
                    u == "WITH" || u.contains("TIMEZONE")
                });
            let precision = params.as_ref().and_then(|p| p.trim().parse().ok());
            DataType::Timestamp {
                with_timezone: with_tz,
                precision,
            }
        }
        "DATETIME" => {
            let precision = params.as_ref().and_then(|p| p.trim().parse().ok());
            DataType::DateTime { precision }
        }
        "INTERVAL" => DataType::Interval,
        "UUID" => DataType::Uuid,
        "JSON" => DataType::Json,
        "JSONB" => DataType::Jsonb,
        "BYTEA" => DataType::Bytea,
        "BINARY" => {
            let len = params.as_ref().and_then(|p| p.trim().parse().ok());
            DataType::Binary(len)
        }
        "VARBINARY" => {
            let len = params.as_ref().and_then(|p| p.trim().parse().ok());
            DataType::VarBinary(len)
        }
        "BLOB" => DataType::Blob,
        "MEDIUMBLOB" => DataType::MediumBlob,
        "LONGBLOB" => DataType::LongBlob,
        "INET" => DataType::Inet,
        "CIDR" => DataType::Cidr,
        "MACADDR" => DataType::MacAddr,
        "YEAR" => DataType::Year,
        "ENUM" => {
            let values = original_params
                .map(|p| {
                    p.split(',')
                        .map(|v| v.trim().trim_matches('\'').trim_matches('"').to_string())
                        .collect()
                })
                .unwrap_or_default();
            DataType::Enum { name: None, values }
        }
        "SET" => {
            let values = original_params
                .map(|p| {
                    p.split(',')
                        .map(|v| v.trim().trim_matches('\'').trim_matches('"').to_string())
                        .collect()
                })
                .unwrap_or_default();
            DataType::Set(values)
        }
        _ => {
            // Check if it's a user-defined enum type (Postgres)
            if dialect == Engine::Postgres {
                DataType::Enum {
                    name: Some(unquote(type_str)),
                    values: vec![],
                }
            } else {
                DataType::Other(type_str.to_string())
            }
        }
    }
}

fn parse_precision_scale(params: &Option<String>) -> (Option<u32>, Option<u32>) {
    match params {
        None => (None, None),
        Some(p) => {
            let parts: Vec<&str> = p.split(',').collect();
            let precision = parts.first().and_then(|s| s.trim().parse().ok());
            let scale = parts.get(1).and_then(|s| s.trim().parse().ok());
            (precision, scale)
        }
    }
}

fn extract_default(part: &str) -> Option<ColumnDefault> {
    let upper = part.to_uppercase();
    let default_pos = upper.find("DEFAULT ")?;
    let after = &part[default_pos + 8..];
    let after = after.trim();

    // Find where the default value ends (next keyword or end)
    let end_keywords = [
        " NOT NULL",
        " NULL",
        " PRIMARY",
        " UNIQUE",
        " CHECK",
        " REFERENCES",
        " CONSTRAINT",
        " AUTO_INCREMENT",
        " ON UPDATE",
        " GENERATED",
        " COMMENT",
    ];
    let upper_after = after.to_uppercase();
    let end_pos = end_keywords
        .iter()
        .filter_map(|kw| upper_after.find(kw))
        .min()
        .unwrap_or(after.len());

    let value = after[..end_pos].trim().trim_end_matches(',');

    if value.is_empty() {
        return None;
    }

    // Determine if it's a literal or expression
    if value.starts_with('\'') && value.ends_with('\'') {
        Some(ColumnDefault::Literal(
            value[1..value.len() - 1].to_string(),
        ))
    } else if value.contains('(')
        || value.to_uppercase() == "CURRENT_TIMESTAMP"
        || value.to_uppercase() == "TRUE"
        || value.to_uppercase() == "FALSE"
    {
        Some(ColumnDefault::Expression(value.to_string()))
    } else {
        Some(ColumnDefault::Literal(value.to_string()))
    }
}

fn parse_create_type(
    stmt: &str,
    _file: &Path,
    _line: usize,
    schema: &mut Schema,
) -> Result<(), ParseError> {
    let upper = stmt.to_uppercase();
    if !upper.contains("AS ENUM") {
        return Ok(());
    }

    // Extract type name
    let after_type = &stmt[stmt.to_uppercase().find("CREATE TYPE").unwrap() + 11..];
    let after_type = after_type.trim();
    let as_pos = match after_type.to_uppercase().find("AS ") {
        Some(p) => p,
        None => return Ok(()),
    };
    let name = unquote(after_type[..as_pos].trim());
    // Handle schema-qualified
    let name = if let Some(dot) = name.rfind('.') {
        name[dot + 1..].to_string()
    } else {
        name
    };

    // Extract values from parentheses
    let paren_start = match after_type.find('(') {
        Some(p) => p,
        None => return Ok(()),
    };
    let paren_end = match after_type.rfind(')') {
        Some(p) => p,
        None => return Ok(()),
    };
    let values_str = &after_type[paren_start + 1..paren_end];
    let values: Vec<String> = values_str
        .split(',')
        .map(|v| v.trim().trim_matches('\'').trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .collect();

    if let Some(existing) = schema.enums.iter_mut().find(|e| e.name == name) {
        existing.values = values;
    } else {
        schema.enums.push(EnumType { name, values });
    }

    Ok(())
}

fn parse_create_index(
    stmt: &str,
    _file: &Path,
    _line: usize,
    schema: &mut Schema,
) -> Result<(), ParseError> {
    let upper = stmt.to_uppercase();
    let unique = upper.contains("UNIQUE");

    // Extract index name
    let idx_keyword = if unique {
        upper
            .find("UNIQUE INDEX")
            .map(|p| p + 12)
            .or_else(|| upper.find("UNIQUE").map(|p| p + 6))
    } else {
        upper.find("INDEX").map(|p| p + 5)
    };
    let after_idx = match idx_keyword {
        Some(pos) => stmt[pos..].trim(),
        None => return Ok(()),
    };

    // Skip IF NOT EXISTS
    let after_idx = if after_idx.to_uppercase().starts_with("IF NOT EXISTS") {
        after_idx[13..].trim()
    } else {
        after_idx
    };

    // Find ON keyword
    let on_pos = match after_idx.to_uppercase().find(" ON ") {
        Some(p) => p,
        None => return Ok(()),
    };

    let index_name = unquote(after_idx[..on_pos].trim());
    let after_on = after_idx[on_pos + 4..].trim();

    // Extract table name
    let paren_pos = match after_on.find('(') {
        Some(p) => p,
        None => return Ok(()),
    };
    let table_name_raw = after_on[..paren_pos].trim();
    let table_name = unquote(table_name_raw);
    let table_name = if let Some(dot) = table_name.rfind('.') {
        table_name[dot + 1..].to_string()
    } else {
        table_name
    };

    // Extract columns
    let cols_end = after_on[paren_pos + 1..]
        .find(')')
        .map(|p| p + paren_pos + 1);
    let cols_str = match cols_end {
        Some(end) => &after_on[paren_pos + 1..end],
        None => return Ok(()),
    };
    let columns: Vec<String> = cols_str
        .split(',')
        .map(|c| {
            let c = c.trim();
            // Remove ASC/DESC suffixes
            let c = c.split_whitespace().next().unwrap_or(c);
            unquote(c)
        })
        .collect();

    // Check for WHERE clause (partial index)
    let predicate = after_on
        .to_uppercase()
        .find(" WHERE ")
        .map(|where_pos| after_on[where_pos + 7..].trim().to_string());

    let index = Index {
        name: Some(index_name),
        columns,
        unique,
        index_type: None,
        predicate,
    };

    if let Some(table) = schema.tables.iter_mut().find(|t| t.name == table_name) {
        table.indexes.push(index);
    }

    Ok(())
}

fn parse_alter_table(
    stmt: &str,
    dialect: Engine,
    file: &Path,
    line: usize,
    schema: &mut Schema,
) -> Result<(), ParseError> {
    let upper = stmt.to_uppercase();
    let after_table = &stmt[upper.find("ALTER TABLE").unwrap() + 11..];
    let after_table = after_table.trim();

    // Skip IF EXISTS
    let after_table = if after_table.to_uppercase().starts_with("IF EXISTS") {
        after_table[9..].trim()
    } else {
        after_table
    };

    // Find the action keyword
    let action_keywords = [
        "ADD COLUMN",
        "DROP COLUMN",
        "ALTER COLUMN",
        "MODIFY COLUMN",
        "ADD CONSTRAINT",
        "DROP CONSTRAINT",
        "ADD FOREIGN KEY",
        "ADD CHECK",
        "ADD UNIQUE",
        "ADD PRIMARY KEY",
        "ADD",
        "DROP",
    ];

    let mut action_pos: Option<(&str, usize)> = None;
    let upper_after = after_table.to_uppercase();
    for kw in &action_keywords {
        if let Some(pos) = upper_after.find(kw) {
            if action_pos.is_none() || pos < action_pos.unwrap().1 {
                action_pos = Some((*kw, pos));
            }
        }
    }

    let (action, pos) = match action_pos {
        Some(a) => a,
        None => return Ok(()),
    };

    let table_name = unquote(after_table[..pos].trim());
    let table_name = if let Some(dot) = table_name.rfind('.') {
        table_name[dot + 1..].to_string()
    } else {
        table_name
    };

    let action_len = action.len();
    let rest = after_table[pos + action_len..].trim();

    let table = match schema.tables.iter_mut().find(|t| t.name == table_name) {
        Some(t) => t,
        None => return Ok(()),
    };

    match action {
        "ADD COLUMN" | "ADD" => {
            if let Some(col) = parse_column_def(rest, dialect, file, line)? {
                table.columns.push(col);
            }
        }
        "DROP COLUMN" | "DROP" => {
            let col_name = unquote(rest.split_whitespace().next().unwrap_or(""));
            table.columns.retain(|c| c.name != col_name);
        }
        "ADD CONSTRAINT" => {
            // Could be FK, CHECK, UNIQUE, etc.
            if let Some(fk) = parse_inline_fk(rest) {
                table.foreign_keys.push(fk);
            } else if let Some(cc) = parse_inline_check(rest) {
                table.check_constraints.push(cc);
            } else if let Some(cols) = extract_paren_list(rest) {
                let name = extract_constraint_name(rest);
                if rest.to_uppercase().contains("UNIQUE") {
                    table.unique_constraints.push(UniqueConstraint {
                        name,
                        columns: cols,
                    });
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn parse_drop_table(stmt: &str, schema: &mut Schema) {
    let upper = stmt.to_uppercase();
    let after = &stmt[upper.find("DROP TABLE").unwrap() + 10..];
    let after = after.trim();
    let after = if after.to_uppercase().starts_with("IF EXISTS") {
        after[9..].trim()
    } else {
        after
    };
    let table_name = unquote(after.split_whitespace().next().unwrap_or(""));
    let table_name = if let Some(dot) = table_name.rfind('.') {
        table_name[dot + 1..].to_string()
    } else {
        table_name
    };
    schema.tables.retain(|t| t.name != table_name);
}

fn parse_drop_index(stmt: &str, schema: &mut Schema) {
    let upper = stmt.to_uppercase();
    let after = &stmt[upper.find("DROP INDEX").unwrap() + 10..];
    let after = after.trim();
    let after = if after.to_uppercase().starts_with("IF EXISTS") {
        after[9..].trim()
    } else {
        after
    };
    let index_name = unquote(after.split_whitespace().next().unwrap_or(""));
    for table in &mut schema.tables {
        table
            .indexes
            .retain(|i| i.name.as_deref() != Some(&index_name));
    }
}

fn parse_inline_fk(part: &str) -> Option<ForeignKey> {
    let upper = part.to_uppercase();

    let name = extract_constraint_name(part);

    let fk_pos = upper.find("FOREIGN KEY")?;
    let after_fk = &part[fk_pos + 11..];

    let cols = extract_paren_list(after_fk)?;

    let ref_pos = after_fk.to_uppercase().find("REFERENCES")?;
    let after_ref = after_fk[ref_pos + 10..].trim();

    let paren_pos = after_ref.find('(')?;
    let ref_table = unquote(after_ref[..paren_pos].trim());
    let ref_table = if let Some(dot) = ref_table.rfind('.') {
        ref_table[dot + 1..].to_string()
    } else {
        ref_table
    };

    let ref_cols_end = after_ref[paren_pos + 1..].find(')')?;
    let ref_cols_str = &after_ref[paren_pos + 1..paren_pos + 1 + ref_cols_end];
    let ref_cols: Vec<String> = ref_cols_str.split(',').map(|c| unquote(c.trim())).collect();

    let on_delete = parse_fk_action(&upper, "ON DELETE");
    let on_update = parse_fk_action(&upper, "ON UPDATE");

    Some(ForeignKey {
        name,
        columns: cols,
        referenced_table: ref_table,
        referenced_schema: None,
        referenced_columns: ref_cols,
        on_delete,
        on_update,
    })
}

fn parse_column_reference(part: &str, col_name: &str) -> Option<ForeignKey> {
    let upper = part.to_uppercase();
    let ref_pos = upper.find("REFERENCES")?;
    let after_ref = part[ref_pos + 10..].trim();

    let paren_pos = after_ref.find('(')?;
    let ref_table = unquote(after_ref[..paren_pos].trim());

    let paren_end = after_ref[paren_pos + 1..].find(')')?;
    let ref_col = unquote(after_ref[paren_pos + 1..paren_pos + 1 + paren_end].trim());

    let on_delete = parse_fk_action(&upper, "ON DELETE");
    let on_update = parse_fk_action(&upper, "ON UPDATE");

    Some(ForeignKey {
        name: None,
        columns: vec![col_name.to_string()],
        referenced_table: ref_table,
        referenced_schema: None,
        referenced_columns: vec![ref_col],
        on_delete,
        on_update,
    })
}

fn parse_fk_action(upper: &str, prefix: &str) -> ForeignKeyAction {
    if let Some(pos) = upper.find(prefix) {
        let after = upper[pos + prefix.len()..].trim();
        if after.starts_with("CASCADE") {
            ForeignKeyAction::Cascade
        } else if after.starts_with("SET NULL") {
            ForeignKeyAction::SetNull
        } else if after.starts_with("SET DEFAULT") {
            ForeignKeyAction::SetDefault
        } else if after.starts_with("RESTRICT") {
            ForeignKeyAction::Restrict
        } else {
            ForeignKeyAction::NoAction
        }
    } else {
        ForeignKeyAction::NoAction
    }
}

fn parse_inline_check(part: &str) -> Option<CheckConstraint> {
    let upper = part.to_uppercase();
    let check_pos = upper.find("CHECK")?;
    let after = &part[check_pos + 5..].trim();
    let paren_start = after.find('(')?;
    let inner = &after[paren_start + 1..];
    let end = find_matching_paren(inner)?;
    let expression = inner[..end].trim().to_string();
    let name = extract_constraint_name(part);

    Some(CheckConstraint { name, expression })
}

fn parse_table_options(rest: &str, options: &mut TableOptions) {
    let upper = rest.to_uppercase();
    if let Some(pos) = upper.find("ENGINE") {
        let after = rest[pos..].split('=').nth(1);
        if let Some(val) = after {
            let val = val.split_whitespace().next().unwrap_or("");
            options.engine = Some(val.to_string());
        }
    }
    if let Some(pos) = upper.find("CHARSET") {
        let after = rest[pos..].split('=').nth(1);
        if let Some(val) = after {
            let val = val.split_whitespace().next().unwrap_or("");
            options.charset = Some(val.to_string());
        }
    }
    if let Some(pos) = upper.find("COLLATE") {
        let after = rest[pos..].split('=').nth(1);
        if let Some(val) = after {
            let val = val.split_whitespace().next().unwrap_or("");
            options.collation = Some(val.to_string());
        }
    }
}

// --- Utility Functions ---

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('`') && s.ends_with('`'))
        || (s.starts_with('[') && s.ends_with(']'))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut string_char = '\'';

    for (i, ch) in s.chars().enumerate() {
        if in_string {
            if ch == string_char {
                in_string = false;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            in_string = true;
            string_char = ch;
            continue;
        }
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
    }
    None
}

fn split_by_comma(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut string_char = '\'';

    for ch in s.chars() {
        if in_string {
            current.push(ch);
            if ch == string_char {
                in_string = false;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            in_string = true;
            string_char = ch;
            current.push(ch);
            continue;
        }
        if ch == '(' {
            depth += 1;
            current.push(ch);
        } else if ch == ')' {
            depth -= 1;
            current.push(ch);
        } else if ch == ',' && depth == 0 {
            parts.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }
    parts
}

fn extract_paren_list(s: &str) -> Option<Vec<String>> {
    let start = s.find('(')?;
    let inner = &s[start + 1..];
    let end = find_matching_paren(inner)?;
    let inner = &inner[..end];
    Some(
        inner
            .split(',')
            .map(|c| unquote(c.trim()))
            .filter(|c| !c.is_empty())
            .collect(),
    )
}

fn extract_constraint_name(s: &str) -> Option<String> {
    let upper = s.to_uppercase();
    if let Some(pos) = upper.find("CONSTRAINT") {
        let after = s[pos + 10..].trim();
        let name_end = after
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(after.len());
        let name = after[..name_end].trim();
        if !name.is_empty() {
            return Some(unquote(name));
        }
    }
    None
}

fn tokenize_column_def(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut string_char = '\'';
    let mut in_parens = 0i32;

    for ch in s.chars() {
        if in_string {
            current.push(ch);
            if ch == string_char {
                in_string = false;
            }
            continue;
        }
        if ch == '\'' || ch == '"' || ch == '`' {
            in_string = true;
            string_char = ch;
            current.push(ch);
            continue;
        }
        if ch == '(' {
            in_parens += 1;
            current.push(ch);
            continue;
        }
        if ch == ')' {
            in_parens -= 1;
            current.push(ch);
            continue;
        }
        if ch.is_whitespace() && in_parens == 0 {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_statements() {
        let sql = "CREATE TABLE foo (id int); CREATE TABLE bar (id int);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_split_statements_with_comments() {
        let sql = "-- comment\nCREATE TABLE foo (id int);\n/* block */\nCREATE TABLE bar (id int);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_parse_simple_create_table() {
        let sql =
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR(100) NOT NULL, email TEXT);";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(schema.tables.len(), 1);
        let table = &schema.tables[0];
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 3);
        assert!(table.primary_key.is_some());
        assert_eq!(table.primary_key.as_ref().unwrap().columns, vec!["id"]);
    }

    #[test]
    fn test_parse_table_with_fk() {
        let sql = r#"
            CREATE TABLE orders (
                id SERIAL PRIMARY KEY,
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
            );
        "#;
        let mut schema = Schema::new("public");
        schema.tables.push(Table::new("users"));
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        let orders = schema.tables.iter().find(|t| t.name == "orders").unwrap();
        assert_eq!(orders.foreign_keys.len(), 1);
        assert_eq!(orders.foreign_keys[0].referenced_table, "users");
        assert_eq!(orders.foreign_keys[0].on_delete, ForeignKeyAction::Cascade);
    }

    #[test]
    fn test_parse_table_with_constraint_fk() {
        let sql = r#"
            CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
            );
        "#;
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        let orders = &schema.tables[0];
        assert_eq!(orders.foreign_keys.len(), 1);
        assert_eq!(orders.foreign_keys[0].name, Some("fk_user".to_string()));
        assert_eq!(orders.foreign_keys[0].on_delete, ForeignKeyAction::SetNull);
    }

    #[test]
    fn test_parse_create_type_enum() {
        let sql = "CREATE TYPE user_role AS ENUM ('admin', 'editor', 'viewer');";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(schema.enums.len(), 1);
        assert_eq!(schema.enums[0].name, "user_role");
        assert_eq!(schema.enums[0].values, vec!["admin", "editor", "viewer"]);
    }

    #[test]
    fn test_parse_create_index() {
        let sql = "CREATE INDEX idx_users_email ON users (email);";
        let mut schema = Schema::new("public");
        schema.tables.push(Table::new("users"));
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        let table = &schema.tables[0];
        assert_eq!(table.indexes.len(), 1);
        assert_eq!(table.indexes[0].name, Some("idx_users_email".to_string()));
        assert!(!table.indexes[0].unique);
    }

    #[test]
    fn test_parse_create_unique_index() {
        let sql = "CREATE UNIQUE INDEX idx_email ON users (email);";
        let mut schema = Schema::new("public");
        schema.tables.push(Table::new("users"));
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert!(schema.tables[0].indexes[0].unique);
    }

    #[test]
    fn test_parse_partial_index() {
        let sql = "CREATE INDEX idx_active ON users (email) WHERE is_active = true;";
        let mut schema = Schema::new("public");
        schema.tables.push(Table::new("users"));
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        let idx = &schema.tables[0].indexes[0];
        assert!(idx.predicate.is_some());
        assert!(idx.predicate.as_ref().unwrap().contains("is_active"));
    }

    #[test]
    fn test_parse_alter_table_add_column() {
        let sql = "CREATE TABLE users (id INTEGER);\nALTER TABLE users ADD COLUMN email TEXT;";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(schema.tables[0].columns.len(), 2);
        assert_eq!(schema.tables[0].columns[1].name, "email");
    }

    #[test]
    fn test_parse_drop_table() {
        let sql = "CREATE TABLE users (id INTEGER);\nDROP TABLE users;";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert!(schema.tables.is_empty());
    }

    #[test]
    fn test_parse_mysql_table() {
        let sql = r#"
            CREATE TABLE users (
                id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                role ENUM('admin','user','guest') DEFAULT 'user',
                token BINARY(32)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
        "#;
        let mut schema = Schema::new("default");
        parse_sql(sql, Engine::Mysql, Path::new("test.sql"), &mut schema).unwrap();

        let table = &schema.tables[0];
        assert_eq!(table.name, "users");
        assert_eq!(table.columns.len(), 4);

        // Check UNSIGNED
        assert!(table.columns[0].unsigned);
        assert!(table.columns[0].auto_increment);

        // Check ENUM
        if let DataType::Enum { name, values } = &table.columns[2].data_type {
            assert!(name.is_none());
            assert_eq!(values, &["admin", "user", "guest"]);
        } else {
            panic!("expected Enum data type");
        }

        // Check BINARY
        assert_eq!(table.columns[3].data_type, DataType::Binary(Some(32)));

        // Check table options
        assert_eq!(table.options.engine, Some("InnoDB".to_string()));
        assert_eq!(table.options.charset, Some("utf8mb4".to_string()));
    }

    #[test]
    fn test_parse_check_constraint() {
        let sql = "CREATE TABLE t (age INTEGER, CHECK (age > 0));";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(schema.tables[0].check_constraints.len(), 1);
        assert!(schema.tables[0].check_constraints[0]
            .expression
            .contains("age > 0"));
    }

    #[test]
    fn test_parse_composite_pk() {
        let sql = "CREATE TABLE t (a INTEGER, b INTEGER, PRIMARY KEY (a, b));";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        let pk = schema.tables[0].primary_key.as_ref().unwrap();
        assert_eq!(pk.columns, vec!["a", "b"]);
    }

    #[test]
    fn test_parse_default_expression() {
        let sql = "CREATE TABLE t (id UUID DEFAULT gen_random_uuid(), created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP);";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(
            schema.tables[0].columns[0].default,
            Some(ColumnDefault::Expression("gen_random_uuid()".to_string()))
        );
        assert_eq!(
            schema.tables[0].columns[1].default,
            Some(ColumnDefault::Expression("CURRENT_TIMESTAMP".to_string()))
        );
    }

    #[test]
    fn test_parse_default_literal() {
        let sql =
            "CREATE TABLE t (name VARCHAR(100) DEFAULT 'unnamed', active BOOLEAN DEFAULT TRUE);";
        let mut schema = Schema::new("public");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(
            schema.tables[0].columns[0].default,
            Some(ColumnDefault::Literal("unnamed".to_string()))
        );
        assert_eq!(
            schema.tables[0].columns[1].default,
            Some(ColumnDefault::Expression("TRUE".to_string()))
        );
    }

    #[test]
    fn test_normalize_statement() {
        let s = "CREATE   TABLE\n  foo  (\n  id  INT\n)";
        let n = normalize_statement(s);
        assert_eq!(n, "CREATE TABLE foo ( id INT )");
    }

    #[test]
    fn test_schema_qualified_table() {
        let sql = "CREATE TABLE analytics.page_views (id INTEGER PRIMARY KEY);";
        let mut schema = Schema::new("analytics");
        parse_sql(sql, Engine::Postgres, Path::new("test.sql"), &mut schema).unwrap();

        assert_eq!(schema.tables[0].name, "page_views");
    }

    #[test]
    fn test_sequential_migrations() {
        let sql1 = "CREATE TABLE users (id INTEGER PRIMARY KEY);";
        let sql2 = "ALTER TABLE users ADD COLUMN email TEXT;";

        let mut schema = Schema::new("public");
        parse_sql(sql1, Engine::Postgres, Path::new("001.sql"), &mut schema).unwrap();
        parse_sql(sql2, Engine::Postgres, Path::new("002.sql"), &mut schema).unwrap();

        assert_eq!(schema.tables[0].columns.len(), 2);
    }
}
