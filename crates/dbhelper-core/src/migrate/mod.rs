use crate::config::Engine;
use crate::diff::{Change, SchemaDiff};
use crate::schema::{
    CheckConstraint, Column, ColumnDefault, DataType, ForeignKey, ForeignKeyAction, Index, Schema,
    Table, UniqueConstraint,
};

/// Generate forward migration SQL from a schema diff.
pub fn generate_forward(diff: &SchemaDiff, schema: &Schema, engine: Engine) -> Vec<String> {
    let mut statements = Vec::new();

    // Dependency ordering: enums first, then tables, then columns/indexes/constraints
    // Phase 1: enums
    for change in &diff.changes {
        match change {
            Change::AddEnum { name } => {
                if let Some(e) = schema.enums.iter().find(|e| e.name == *name) {
                    let values: Vec<String> = e
                        .values
                        .iter()
                        .map(|v| format!("'{}'", escape(v)))
                        .collect();
                    statements.push(format!(
                        "CREATE TYPE {} AS ENUM ({});",
                        quote_ident(name, engine),
                        values.join(", ")
                    ));
                }
            }
            Change::AlterEnum { name, description } => {
                // Postgres ALTER TYPE ... ADD VALUE
                if engine == Engine::Postgres && description.contains("added values:") {
                    if let Some(added) = description.strip_prefix("added values: ") {
                        let added = added.split("; ").next().unwrap_or(added);
                        for val in added.split(", ") {
                            statements.push(format!(
                                "ALTER TYPE {} ADD VALUE '{}';",
                                quote_ident(name, engine),
                                escape(val)
                            ));
                        }
                    }
                }
            }
            Change::DropEnum { name } => {
                statements.push(format!("DROP TYPE {};", quote_ident(name, engine)));
            }
            _ => {}
        }
    }

    // Phase 2: create tables (before columns/indexes that reference them)
    for change in &diff.changes {
        if let Change::AddTable(name) = change {
            if let Some(table) = schema.tables.iter().find(|t| t.name == *name) {
                statements.push(generate_create_table(table, engine));
            }
        }
    }

    // Phase 3: alter existing tables (columns, indexes, constraints, PKs)
    for change in &diff.changes {
        match change {
            Change::AddColumn { table, column } => {
                if let Some(t) = schema.tables.iter().find(|t| t.name == *table) {
                    if let Some(col) = t.columns.iter().find(|c| c.name == *column) {
                        statements.push(format!(
                            "ALTER TABLE {} ADD COLUMN {};",
                            quote_ident(table, engine),
                            column_def(col, engine)
                        ));
                    }
                }
            }
            Change::DropColumn { table, column } => {
                statements.push(format!(
                    "ALTER TABLE {} DROP COLUMN {};",
                    quote_ident(table, engine),
                    quote_ident(column, engine)
                ));
            }
            Change::AlterColumn {
                table,
                column,
                description,
            } => {
                statements.extend(generate_alter_column(table, column, description, engine));
            }
            Change::AddIndex { table, index } => {
                if let Some(t) = schema.tables.iter().find(|t| t.name == *table) {
                    if let Some(idx) = t.indexes.iter().find(|i| {
                        i.name.as_deref() == Some(index.as_str()) || i.columns.join(",") == *index
                    }) {
                        statements.push(generate_create_index(idx, table, engine));
                    }
                }
            }
            Change::DropIndex { table: _, index } => {
                if engine == Engine::Postgres {
                    statements.push(format!("DROP INDEX {};", quote_ident(index, engine)));
                } else {
                    // MySQL requires table name, but we don't have it in DropIndex
                    // We use the table from the change
                    statements.push(format!("DROP INDEX {};", quote_ident(index, engine)));
                }
            }
            Change::AddForeignKey { table, name } => {
                if let Some(t) = schema.tables.iter().find(|t| t.name == *table) {
                    if let Some(fk) = t
                        .foreign_keys
                        .iter()
                        .find(|f| f.name.as_deref() == Some(name.as_str()))
                    {
                        statements.push(format!(
                            "ALTER TABLE {} ADD {};",
                            quote_ident(table, engine),
                            fk_constraint(fk, engine)
                        ));
                    }
                }
            }
            Change::DropForeignKey { table, name } => {
                if engine == Engine::Postgres {
                    statements.push(format!(
                        "ALTER TABLE {} DROP CONSTRAINT {};",
                        quote_ident(table, engine),
                        quote_ident(name, engine)
                    ));
                } else {
                    statements.push(format!(
                        "ALTER TABLE {} DROP FOREIGN KEY {};",
                        quote_ident(table, engine),
                        quote_ident(name, engine)
                    ));
                }
            }
            Change::AddCheckConstraint { table, name } => {
                if let Some(t) = schema.tables.iter().find(|t| t.name == *table) {
                    if let Some(cc) = t
                        .check_constraints
                        .iter()
                        .find(|c| c.name.as_deref() == Some(name.as_str()) || c.expression == *name)
                    {
                        let constraint_name = cc
                            .name
                            .as_ref()
                            .map(|n| format!("CONSTRAINT {} ", quote_ident(n, engine)))
                            .unwrap_or_default();
                        statements.push(format!(
                            "ALTER TABLE {} ADD {}CHECK ({});",
                            quote_ident(table, engine),
                            constraint_name,
                            cc.expression
                        ));
                    }
                }
            }
            Change::DropCheckConstraint { table, name } => {
                statements.push(format!(
                    "ALTER TABLE {} DROP CONSTRAINT {};",
                    quote_ident(table, engine),
                    quote_ident(name, engine)
                ));
            }
            Change::AddUniqueConstraint { table, name } => {
                if let Some(t) = schema.tables.iter().find(|t| t.name == *table) {
                    if let Some(uc) = t.unique_constraints.iter().find(|u| {
                        u.name.as_deref() == Some(name.as_str()) || u.columns.join(",") == *name
                    }) {
                        let constraint_name = uc
                            .name
                            .as_ref()
                            .map(|n| format!("CONSTRAINT {} ", quote_ident(n, engine)))
                            .unwrap_or_default();
                        let cols: Vec<String> =
                            uc.columns.iter().map(|c| quote_ident(c, engine)).collect();
                        statements.push(format!(
                            "ALTER TABLE {} ADD {}UNIQUE ({});",
                            quote_ident(table, engine),
                            constraint_name,
                            cols.join(", ")
                        ));
                    }
                }
            }
            Change::DropUniqueConstraint { table, name } => {
                if engine == Engine::Postgres {
                    statements.push(format!(
                        "ALTER TABLE {} DROP CONSTRAINT {};",
                        quote_ident(table, engine),
                        quote_ident(name, engine)
                    ));
                } else {
                    statements.push(format!(
                        "ALTER TABLE {} DROP INDEX {};",
                        quote_ident(table, engine),
                        quote_ident(name, engine)
                    ));
                }
            }
            Change::ChangePrimaryKey { table } => {
                if let Some(t) = schema.tables.iter().find(|t| t.name == *table) {
                    statements.push(format!(
                        "ALTER TABLE {} DROP CONSTRAINT {}_pkey;",
                        quote_ident(table, engine),
                        table
                    ));
                    if let Some(pk) = &t.primary_key {
                        let cols: Vec<String> =
                            pk.columns.iter().map(|c| quote_ident(c, engine)).collect();
                        statements.push(format!(
                            "ALTER TABLE {} ADD PRIMARY KEY ({});",
                            quote_ident(table, engine),
                            cols.join(", ")
                        ));
                    }
                }
            }
            // Table-level changes handled in phase 2
            Change::AddTable(_)
            | Change::AddEnum { .. }
            | Change::DropEnum { .. }
            | Change::AlterEnum { .. } => {}
            Change::DropTable(_) => {} // handled below
        }
    }

    // Phase 4: drop tables last (after dropping FKs that might reference them)
    for change in &diff.changes {
        if let Change::DropTable(name) = change {
            statements.push(format!("DROP TABLE {};", quote_ident(name, engine)));
        }
    }

    statements
}

/// Generate rollback migration SQL (reverse of forward).
pub fn generate_rollback(diff: &SchemaDiff, old_schema: &Schema, engine: Engine) -> Vec<String> {
    let mut reverse_changes = Vec::new();

    for change in diff.changes.iter().rev() {
        match change {
            Change::AddTable(name) => {
                reverse_changes.push(Change::DropTable(name.clone()));
            }
            Change::DropTable(name) => {
                reverse_changes.push(Change::AddTable(name.clone()));
            }
            Change::AddColumn { table, column } => {
                reverse_changes.push(Change::DropColumn {
                    table: table.clone(),
                    column: column.clone(),
                });
            }
            Change::DropColumn { table, column } => {
                reverse_changes.push(Change::AddColumn {
                    table: table.clone(),
                    column: column.clone(),
                });
            }
            Change::AddIndex { table, index } => {
                reverse_changes.push(Change::DropIndex {
                    table: table.clone(),
                    index: index.clone(),
                });
            }
            Change::DropIndex { table, index } => {
                reverse_changes.push(Change::AddIndex {
                    table: table.clone(),
                    index: index.clone(),
                });
            }
            Change::AddForeignKey { table, name } => {
                reverse_changes.push(Change::DropForeignKey {
                    table: table.clone(),
                    name: name.clone(),
                });
            }
            Change::DropForeignKey { table, name } => {
                reverse_changes.push(Change::AddForeignKey {
                    table: table.clone(),
                    name: name.clone(),
                });
            }
            Change::AddCheckConstraint { table, name } => {
                reverse_changes.push(Change::DropCheckConstraint {
                    table: table.clone(),
                    name: name.clone(),
                });
            }
            Change::DropCheckConstraint { table, name } => {
                reverse_changes.push(Change::AddCheckConstraint {
                    table: table.clone(),
                    name: name.clone(),
                });
            }
            Change::AddUniqueConstraint { table, name } => {
                reverse_changes.push(Change::DropUniqueConstraint {
                    table: table.clone(),
                    name: name.clone(),
                });
            }
            Change::DropUniqueConstraint { table, name } => {
                reverse_changes.push(Change::AddUniqueConstraint {
                    table: table.clone(),
                    name: name.clone(),
                });
            }
            Change::AddEnum { name } => {
                reverse_changes.push(Change::DropEnum { name: name.clone() });
            }
            Change::DropEnum { name } => {
                reverse_changes.push(Change::AddEnum { name: name.clone() });
            }
            // AlterColumn and AlterEnum rollbacks require the old state
            Change::AlterColumn { table, column, .. } => {
                // Can't generate precise rollback without old column definition
                reverse_changes.push(Change::AlterColumn {
                    table: table.clone(),
                    column: column.clone(),
                    description: "-- requires manual rollback".to_string(),
                });
            }
            Change::AlterEnum { name, .. } => {
                reverse_changes.push(Change::AlterEnum {
                    name: name.clone(),
                    description: "-- requires manual rollback".to_string(),
                });
            }
            Change::ChangePrimaryKey { table } => {
                reverse_changes.push(Change::ChangePrimaryKey {
                    table: table.clone(),
                });
            }
        }
    }

    let reverse_diff = SchemaDiff {
        changes: reverse_changes,
    };
    generate_forward(&reverse_diff, old_schema, engine)
}

/// Generate a full CREATE TABLE statement.
fn generate_create_table(table: &Table, engine: Engine) -> String {
    let mut parts = Vec::new();

    for col in &table.columns {
        parts.push(format!("  {}", column_def(col, engine)));
    }

    if let Some(pk) = &table.primary_key {
        let cols: Vec<String> = pk.columns.iter().map(|c| quote_ident(c, engine)).collect();
        let name_part = pk
            .name
            .as_ref()
            .map(|n| format!("CONSTRAINT {} ", quote_ident(n, engine)))
            .unwrap_or_default();
        parts.push(format!("  {}PRIMARY KEY ({})", name_part, cols.join(", ")));
    }

    for uc in &table.unique_constraints {
        parts.push(format!("  {}", unique_constraint_def(uc, engine)));
    }

    for cc in &table.check_constraints {
        parts.push(format!("  {}", check_constraint_def(cc, engine)));
    }

    for fk in &table.foreign_keys {
        parts.push(format!("  {}", fk_constraint(fk, engine)));
    }

    let mut sql = format!(
        "CREATE TABLE {} (\n{}\n)",
        quote_ident(&table.name, engine),
        parts.join(",\n")
    );

    // MySQL table options
    if engine == Engine::Mysql {
        if let Some(eng) = &table.options.engine {
            sql.push_str(&format!(" ENGINE={eng}"));
        }
        if let Some(charset) = &table.options.charset {
            sql.push_str(&format!(" DEFAULT CHARSET={charset}"));
        }
        if let Some(collation) = &table.options.collation {
            sql.push_str(&format!(" COLLATE={collation}"));
        }
    }

    sql.push(';');
    sql
}

/// Generate a column definition string.
fn column_def(col: &Column, engine: Engine) -> String {
    let mut def = format!(
        "{} {}",
        quote_ident(&col.name, engine),
        type_sql(&col.data_type, engine)
    );

    if col.unsigned && engine == Engine::Mysql {
        def.push_str(" UNSIGNED");
    }

    if !col.nullable {
        def.push_str(" NOT NULL");
    }

    if col.auto_increment {
        match engine {
            Engine::Postgres => {
                // For Postgres, serial types handle auto-increment
                // but if we're generating ALTER TABLE ADD COLUMN, we use GENERATED
            }
            Engine::Mysql => def.push_str(" AUTO_INCREMENT"),
        }
    }

    if let Some(default) = &col.default {
        match default {
            ColumnDefault::Literal(v) => def.push_str(&format!(" DEFAULT '{}'", escape(v))),
            ColumnDefault::Expression(e) => def.push_str(&format!(" DEFAULT {e}")),
        }
    }

    def
}

/// Convert a DataType to SQL string for the given dialect.
fn type_sql(dt: &DataType, engine: Engine) -> String {
    match dt {
        DataType::SmallInt => "SMALLINT".to_string(),
        DataType::Integer => "INTEGER".to_string(),
        DataType::BigInt => "BIGINT".to_string(),
        DataType::TinyInt => "TINYINT".to_string(),
        DataType::MediumInt => "MEDIUMINT".to_string(),
        DataType::Real => {
            if engine == Engine::Mysql {
                "FLOAT".to_string()
            } else {
                "REAL".to_string()
            }
        }
        DataType::DoublePrecision => {
            if engine == Engine::Mysql {
                "DOUBLE".to_string()
            } else {
                "DOUBLE PRECISION".to_string()
            }
        }
        DataType::Numeric { precision, scale } => match (precision, scale) {
            (Some(p), Some(s)) => format!("NUMERIC({p},{s})"),
            (Some(p), None) => format!("NUMERIC({p})"),
            _ => "NUMERIC".to_string(),
        },
        DataType::Varchar(len) => match len {
            Some(n) => format!("VARCHAR({n})"),
            None => "VARCHAR".to_string(),
        },
        DataType::Char(len) => match len {
            Some(n) => format!("CHAR({n})"),
            None => "CHAR".to_string(),
        },
        DataType::Text => "TEXT".to_string(),
        DataType::MediumText => "MEDIUMTEXT".to_string(),
        DataType::LongText => "LONGTEXT".to_string(),
        DataType::Boolean => {
            if engine == Engine::Mysql {
                "TINYINT(1)".to_string()
            } else {
                "BOOLEAN".to_string()
            }
        }
        DataType::Date => "DATE".to_string(),
        DataType::Time => "TIME".to_string(),
        DataType::Timestamp {
            with_timezone,
            precision,
        } => {
            let mut s = "TIMESTAMP".to_string();
            if let Some(p) = precision {
                s.push_str(&format!("({p})"));
            }
            if *with_timezone && engine == Engine::Postgres {
                s.push_str(" WITH TIME ZONE");
            }
            s
        }
        DataType::DateTime { precision } => match precision {
            Some(p) => format!("DATETIME({p})"),
            None => "DATETIME".to_string(),
        },
        DataType::Interval => "INTERVAL".to_string(),
        DataType::Uuid => {
            if engine == Engine::Postgres {
                "UUID".to_string()
            } else {
                "CHAR(36)".to_string()
            }
        }
        DataType::Json => "JSON".to_string(),
        DataType::Jsonb => {
            if engine == Engine::Postgres {
                "JSONB".to_string()
            } else {
                "JSON".to_string()
            }
        }
        DataType::Bytea => {
            if engine == Engine::Postgres {
                "BYTEA".to_string()
            } else {
                "BLOB".to_string()
            }
        }
        DataType::Binary(len) => match len {
            Some(n) => format!("BINARY({n})"),
            None => "BINARY".to_string(),
        },
        DataType::VarBinary(len) => match len {
            Some(n) => format!("VARBINARY({n})"),
            None => "VARBINARY".to_string(),
        },
        DataType::Blob => "BLOB".to_string(),
        DataType::MediumBlob => "MEDIUMBLOB".to_string(),
        DataType::LongBlob => "LONGBLOB".to_string(),
        DataType::Inet => "INET".to_string(),
        DataType::Cidr => "CIDR".to_string(),
        DataType::MacAddr => "MACADDR".to_string(),
        DataType::Enum { name, values } => {
            if engine == Engine::Postgres {
                // Use named type
                if let Some(n) = name {
                    n.clone()
                } else {
                    "TEXT".to_string()
                }
            } else {
                // MySQL inline ENUM
                let vals: Vec<String> = values.iter().map(|v| format!("'{}'", escape(v))).collect();
                format!("ENUM({})", vals.join(", "))
            }
        }
        DataType::Set(values) => {
            let vals: Vec<String> = values.iter().map(|v| format!("'{}'", escape(v))).collect();
            format!("SET({})", vals.join(", "))
        }
        DataType::Array(inner) => {
            format!("{}[]", type_sql(inner, engine))
        }
        DataType::Year => "YEAR".to_string(),
        DataType::Other(s) => s.clone(),
    }
}

/// Generate ALTER COLUMN statements from a description string.
fn generate_alter_column(
    table: &str,
    column: &str,
    description: &str,
    engine: Engine,
) -> Vec<String> {
    let mut stmts = Vec::new();

    if description.contains("-- requires manual rollback") {
        stmts.push(format!(
            "-- ALTER COLUMN {}.{} requires manual migration",
            table, column
        ));
        return stmts;
    }

    // Parse type change: "type: X -> Y"
    for part in description.split(", ") {
        if let Some(type_change) = part.strip_prefix("type: ") {
            if let Some((_old, new)) = type_change.split_once(" -> ") {
                if engine == Engine::Postgres {
                    stmts.push(format!(
                        "ALTER TABLE {} ALTER COLUMN {} TYPE {} USING {}::{};",
                        quote_ident(table, engine),
                        quote_ident(column, engine),
                        new,
                        quote_ident(column, engine),
                        new
                    ));
                } else {
                    stmts.push(format!(
                        "ALTER TABLE {} MODIFY COLUMN {} {};",
                        quote_ident(table, engine),
                        quote_ident(column, engine),
                        new
                    ));
                }
            }
        } else if let Some(nullable_change) = part.strip_prefix("nullable: ") {
            if let Some((_old, new)) = nullable_change.split_once(" -> ") {
                if engine == Engine::Postgres {
                    if new == "true" {
                        stmts.push(format!(
                            "ALTER TABLE {} ALTER COLUMN {} DROP NOT NULL;",
                            quote_ident(table, engine),
                            quote_ident(column, engine)
                        ));
                    } else {
                        stmts.push(format!(
                            "ALTER TABLE {} ALTER COLUMN {} SET NOT NULL;",
                            quote_ident(table, engine),
                            quote_ident(column, engine)
                        ));
                    }
                }
                // MySQL MODIFY COLUMN would need full column def
            }
        } else if let Some(default_change) = part.strip_prefix("default: ") {
            if let Some((_old, new)) = default_change.split_once(" -> ") {
                if engine == Engine::Postgres {
                    if new == "none" {
                        stmts.push(format!(
                            "ALTER TABLE {} ALTER COLUMN {} DROP DEFAULT;",
                            quote_ident(table, engine),
                            quote_ident(column, engine)
                        ));
                    } else {
                        stmts.push(format!(
                            "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT {};",
                            quote_ident(table, engine),
                            quote_ident(column, engine),
                            new
                        ));
                    }
                }
            }
        }
    }

    stmts
}

fn generate_create_index(idx: &Index, table: &str, engine: Engine) -> String {
    let unique = if idx.unique { "UNIQUE " } else { "" };
    let name = idx
        .name
        .as_ref()
        .map(|n| format!("{} ", quote_ident(n, engine)))
        .unwrap_or_default();
    let cols: Vec<String> = idx.columns.iter().map(|c| quote_ident(c, engine)).collect();

    let mut sql = format!(
        "CREATE {}INDEX {}ON {} ({});",
        unique,
        name,
        quote_ident(table, engine),
        cols.join(", ")
    );

    // Partial index (Postgres)
    if let Some(pred) = &idx.predicate {
        // Remove trailing semicolon, add WHERE clause
        sql.pop(); // remove ;
        sql.push_str(&format!(" WHERE {pred};"));
    }

    sql
}

fn fk_constraint(fk: &ForeignKey, engine: Engine) -> String {
    let name_part = fk
        .name
        .as_ref()
        .map(|n| format!("CONSTRAINT {} ", quote_ident(n, engine)))
        .unwrap_or_default();
    let cols: Vec<String> = fk.columns.iter().map(|c| quote_ident(c, engine)).collect();
    let ref_cols: Vec<String> = fk
        .referenced_columns
        .iter()
        .map(|c| quote_ident(c, engine))
        .collect();

    let ref_table = if let Some(ref_schema) = &fk.referenced_schema {
        format!(
            "{}.{}",
            quote_ident(ref_schema, engine),
            quote_ident(&fk.referenced_table, engine)
        )
    } else {
        quote_ident(&fk.referenced_table, engine)
    };

    let mut sql = format!(
        "{}FOREIGN KEY ({}) REFERENCES {} ({})",
        name_part,
        cols.join(", "),
        ref_table,
        ref_cols.join(", ")
    );

    if fk.on_delete != ForeignKeyAction::NoAction {
        sql.push_str(&format!(" ON DELETE {}", fk.on_delete));
    }
    if fk.on_update != ForeignKeyAction::NoAction {
        sql.push_str(&format!(" ON UPDATE {}", fk.on_update));
    }

    sql
}

fn unique_constraint_def(uc: &UniqueConstraint, engine: Engine) -> String {
    let name_part = uc
        .name
        .as_ref()
        .map(|n| format!("CONSTRAINT {} ", quote_ident(n, engine)))
        .unwrap_or_default();
    let cols: Vec<String> = uc.columns.iter().map(|c| quote_ident(c, engine)).collect();
    format!("{}UNIQUE ({})", name_part, cols.join(", "))
}

fn check_constraint_def(cc: &CheckConstraint, engine: Engine) -> String {
    let name_part = cc
        .name
        .as_ref()
        .map(|n| format!("CONSTRAINT {} ", quote_ident(n, engine)))
        .unwrap_or_default();
    format!("{}CHECK ({})", name_part, cc.expression)
}

/// Quote an identifier for the given engine.
fn quote_ident(name: &str, engine: Engine) -> String {
    match engine {
        Engine::Postgres => format!("\"{name}\""),
        Engine::Mysql => format!("`{name}`"),
    }
}

fn escape(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff;
    use crate::schema::*;

    fn make_column(name: &str, data_type: DataType) -> Column {
        Column {
            name: name.to_string(),
            data_type,
            nullable: false,
            default: None,
            auto_increment: false,
            unsigned: false,
        }
    }

    #[test]
    fn test_generate_create_table() {
        let table = Table {
            columns: vec![
                Column {
                    auto_increment: true,
                    ..make_column("id", DataType::BigInt)
                },
                Column {
                    nullable: true,
                    ..make_column("name", DataType::Varchar(Some(255)))
                },
            ],
            primary_key: Some(PrimaryKey {
                name: None,
                columns: vec!["id".to_string()],
            }),
            ..Table::new("users")
        };

        let sql = generate_create_table(&table, Engine::Postgres);
        assert!(sql.contains("CREATE TABLE \"users\""));
        assert!(sql.contains("\"id\" BIGINT NOT NULL"));
        assert!(sql.contains("\"name\" VARCHAR(255)"));
        assert!(sql.contains("PRIMARY KEY (\"id\")"));
        // name column should NOT have NOT NULL (it's nullable)
        assert!(
            !sql.contains("\"name\" VARCHAR(255) NOT NULL"),
            "name should be nullable, but got: {sql}"
        );
    }

    #[test]
    fn test_generate_create_table_mysql() {
        let table = Table {
            columns: vec![Column {
                auto_increment: true,
                ..make_column("id", DataType::Integer)
            }],
            primary_key: Some(PrimaryKey {
                name: None,
                columns: vec!["id".to_string()],
            }),
            options: TableOptions {
                engine: Some("InnoDB".to_string()),
                charset: Some("utf8mb4".to_string()),
                collation: None,
            },
            ..Table::new("users")
        };

        let sql = generate_create_table(&table, Engine::Mysql);
        assert!(sql.contains("CREATE TABLE `users`"));
        assert!(sql.contains("AUTO_INCREMENT"));
        assert!(sql.contains("ENGINE=InnoDB"));
        assert!(sql.contains("DEFAULT CHARSET=utf8mb4"));
    }

    #[test]
    fn test_forward_add_table() {
        let from = Schema::new("public");
        let to = Schema {
            tables: vec![Table {
                columns: vec![make_column("id", DataType::Integer)],
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string()],
                }),
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("CREATE TABLE"));
    }

    #[test]
    fn test_forward_drop_table() {
        let from = Schema {
            tables: vec![Table::new("old_table")],
            ..Schema::new("public")
        };
        let to = Schema::new("public");

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("DROP TABLE"));
        assert!(stmts[0].contains("\"old_table\""));
    }

    #[test]
    fn test_forward_add_column() {
        let from = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                columns: vec![make_column("email", DataType::Text)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("ALTER TABLE"));
        assert!(stmts[0].contains("ADD COLUMN"));
        assert!(stmts[0].contains("\"email\" TEXT"));
    }

    #[test]
    fn test_forward_drop_column() {
        let from = Schema {
            tables: vec![Table {
                columns: vec![make_column("old_col", DataType::Text)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("DROP COLUMN"));
    }

    #[test]
    fn test_forward_add_index() {
        let from = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                indexes: vec![Index {
                    name: Some("idx_email".to_string()),
                    columns: vec!["email".to_string()],
                    unique: true,
                    index_type: None,
                    predicate: None,
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("CREATE UNIQUE INDEX"));
        assert!(stmts[0].contains("\"idx_email\""));
    }

    #[test]
    fn test_forward_partial_index() {
        let from = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                indexes: vec![Index {
                    name: Some("idx_active".to_string()),
                    columns: vec!["is_active".to_string()],
                    unique: false,
                    index_type: None,
                    predicate: Some("is_active = true".to_string()),
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("WHERE is_active = true"));
    }

    #[test]
    fn test_forward_add_enum() {
        let from = Schema::new("public");
        let to = Schema {
            enums: vec![EnumType {
                name: "user_role".to_string(),
                values: vec!["admin".to_string(), "user".to_string()],
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("CREATE TYPE"));
        assert!(stmts[0].contains("'admin', 'user'"));
    }

    #[test]
    fn test_forward_add_fk() {
        let from = Schema {
            tables: vec![Table::new("orders")],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                foreign_keys: vec![ForeignKey {
                    name: Some("fk_user".to_string()),
                    columns: vec!["user_id".to_string()],
                    referenced_table: "users".to_string(),
                    referenced_schema: None,
                    referenced_columns: vec!["id".to_string()],
                    on_delete: ForeignKeyAction::Cascade,
                    on_update: ForeignKeyAction::NoAction,
                }],
                ..Table::new("orders")
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let stmts = generate_forward(&d, &to, Engine::Postgres);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("FOREIGN KEY"));
        assert!(stmts[0].contains("REFERENCES"));
        assert!(stmts[0].contains("ON DELETE CASCADE"));
    }

    #[test]
    fn test_rollback_add_table() {
        let from = Schema::new("public");
        let to = Schema {
            tables: vec![Table {
                columns: vec![make_column("id", DataType::Integer)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };

        let d = diff::diff(&from, &to);
        let rollback = generate_rollback(&d, &from, Engine::Postgres);
        assert_eq!(rollback.len(), 1);
        assert!(rollback[0].contains("DROP TABLE"));
    }

    #[test]
    fn test_rollback_drop_table() {
        let from = Schema {
            tables: vec![Table {
                columns: vec![make_column("id", DataType::Integer)],
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string()],
                }),
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let to = Schema::new("public");

        let d = diff::diff(&from, &to);
        let rollback = generate_rollback(&d, &from, Engine::Postgres);
        assert_eq!(rollback.len(), 1);
        assert!(rollback[0].contains("CREATE TABLE"));
    }

    #[test]
    fn test_mysql_quoting() {
        let table = Table {
            columns: vec![make_column("name", DataType::Varchar(Some(100)))],
            ..Table::new("users")
        };
        let sql = generate_create_table(&table, Engine::Mysql);
        assert!(sql.contains("`users`"));
        assert!(sql.contains("`name`"));
    }

    #[test]
    fn test_type_sql_postgres() {
        assert_eq!(type_sql(&DataType::Boolean, Engine::Postgres), "BOOLEAN");
        assert_eq!(type_sql(&DataType::Uuid, Engine::Postgres), "UUID");
        assert_eq!(type_sql(&DataType::Jsonb, Engine::Postgres), "JSONB");
        assert_eq!(type_sql(&DataType::Bytea, Engine::Postgres), "BYTEA");
    }

    #[test]
    fn test_type_sql_mysql() {
        assert_eq!(type_sql(&DataType::Boolean, Engine::Mysql), "TINYINT(1)");
        assert_eq!(type_sql(&DataType::Uuid, Engine::Mysql), "CHAR(36)");
        assert_eq!(type_sql(&DataType::Jsonb, Engine::Mysql), "JSON");
        assert_eq!(type_sql(&DataType::Bytea, Engine::Mysql), "BLOB");
    }

    #[test]
    fn test_nullable_column_no_not_null() {
        let col = Column {
            nullable: true,
            ..make_column("bio", DataType::Text)
        };
        let def = column_def(&col, Engine::Postgres);
        assert!(!def.contains("NOT NULL"));
    }

    #[test]
    fn test_column_with_default() {
        let col = Column {
            default: Some(ColumnDefault::Expression("now()".to_string())),
            ..make_column(
                "created_at",
                DataType::Timestamp {
                    with_timezone: true,
                    precision: None,
                },
            )
        };
        let def = column_def(&col, Engine::Postgres);
        assert!(def.contains("DEFAULT now()"));
    }
}
