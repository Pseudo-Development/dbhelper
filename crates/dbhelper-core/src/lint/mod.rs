use crate::schema::{DataType, Schema, Table};
use serde::{Deserialize, Serialize};

/// Severity level for lint warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintWarning {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub table: Option<String>,
    pub column: Option<String>,
}

/// Trait for individual lint rules.
pub trait LintRule {
    /// Rule identifier (e.g. "missing-primary-key").
    fn name(&self) -> &str;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// Check the schema and return warnings.
    fn check(&self, schema: &Schema) -> Vec<LintWarning>;
}

/// Configuration for the lint engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LintConfig {
    /// Rules to disable by name.
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    /// Naming convention: "snake_case" or "camelCase".
    #[serde(default = "default_naming_convention")]
    pub naming_convention: String,
    /// Threshold for "too many enum values".
    #[serde(default = "default_enum_threshold")]
    pub enum_value_threshold: usize,
    /// Whether to require created_at/updated_at timestamps.
    #[serde(default)]
    pub require_timestamps: bool,
}

fn default_naming_convention() -> String {
    "snake_case".to_string()
}

fn default_enum_threshold() -> usize {
    20
}

/// Run all lint rules against a schema with the given configuration.
pub fn lint(schema: &Schema, config: &LintConfig) -> Vec<LintWarning> {
    let rules: Vec<Box<dyn LintRule>> = vec![
        Box::new(MissingPrimaryKey),
        Box::new(MissingForeignKeyIndex),
        Box::new(NullableUniqueColumn),
        Box::new(UnboundedTextColumn),
        Box::new(ForeignKeyWithoutAction),
        Box::new(NamingConvention {
            convention: config.naming_convention.clone(),
        }),
        Box::new(DuplicateIndex),
        Box::new(EnumTooManyValues {
            threshold: config.enum_value_threshold,
        }),
        Box::new(MissingTimestamps {
            enabled: config.require_timestamps,
        }),
        Box::new(BooleanColumnIndex),
    ];

    let mut warnings = Vec::new();
    for rule in &rules {
        if config.disabled_rules.contains(&rule.name().to_string()) {
            continue;
        }
        warnings.extend(rule.check(schema));
    }
    warnings
}

/// Run lint with default configuration.
pub fn lint_default(schema: &Schema) -> Vec<LintWarning> {
    lint(schema, &LintConfig::default())
}

// --- Individual Rules ---

/// Rule 1: Missing primary key on a table.
struct MissingPrimaryKey;

impl LintRule for MissingPrimaryKey {
    fn name(&self) -> &str {
        "missing-primary-key"
    }
    fn description(&self) -> &str {
        "Tables should have a primary key"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        schema
            .tables
            .iter()
            .filter(|t| t.primary_key.is_none())
            .map(|t| LintWarning {
                rule: self.name().to_string(),
                severity: Severity::Error,
                message: format!("table '{}' has no primary key", t.name),
                table: Some(t.name.clone()),
                column: None,
            })
            .collect()
    }
}

/// Rule 2: Missing index on foreign key columns.
struct MissingForeignKeyIndex;

impl LintRule for MissingForeignKeyIndex {
    fn name(&self) -> &str {
        "missing-fk-index"
    }
    fn description(&self) -> &str {
        "Foreign key columns should be indexed for join performance"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for table in &schema.tables {
            let indexed_cols = table_indexed_columns_set(table);
            for fk in &table.foreign_keys {
                for col in &fk.columns {
                    if !indexed_cols.contains(col.as_str()) {
                        warnings.push(LintWarning {
                            rule: self.name().to_string(),
                            severity: Severity::Warning,
                            message: format!(
                                "column '{}.{}' is a foreign key but has no index",
                                table.name, col
                            ),
                            table: Some(table.name.clone()),
                            column: Some(col.clone()),
                        });
                    }
                }
            }
        }
        warnings
    }
}

/// Rule 3: Nullable column in a unique constraint.
struct NullableUniqueColumn;

impl LintRule for NullableUniqueColumn {
    fn name(&self) -> &str {
        "nullable-unique"
    }
    fn description(&self) -> &str {
        "Nullable columns in unique constraints can have multiple NULLs"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for table in &schema.tables {
            for uc in &table.unique_constraints {
                for col_name in &uc.columns {
                    if let Some(col) = table.columns.iter().find(|c| c.name == *col_name) {
                        if col.nullable {
                            warnings.push(LintWarning {
                                rule: self.name().to_string(),
                                severity: Severity::Warning,
                                message: format!(
                                    "column '{}.{}' is nullable but part of a unique constraint",
                                    table.name, col_name
                                ),
                                table: Some(table.name.clone()),
                                column: Some(col_name.clone()),
                            });
                        }
                    }
                }
            }
        }
        warnings
    }
}

/// Rule 4: Text/blob column without a length limit (prefer varchar).
struct UnboundedTextColumn;

impl LintRule for UnboundedTextColumn {
    fn name(&self) -> &str {
        "unbounded-text"
    }
    fn description(&self) -> &str {
        "Prefer varchar with a length limit over unbounded text types"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for table in &schema.tables {
            for col in &table.columns {
                if matches!(
                    col.data_type,
                    DataType::Text | DataType::MediumText | DataType::LongText
                ) {
                    warnings.push(LintWarning {
                        rule: self.name().to_string(),
                        severity: Severity::Info,
                        message: format!(
                            "column '{}.{}' uses unbounded text type '{}'; consider varchar with a length limit",
                            table.name, col.name, col.data_type
                        ),
                        table: Some(table.name.clone()),
                        column: Some(col.name.clone()),
                    });
                }
            }
        }
        warnings
    }
}

/// Rule 5: Foreign key without explicit ON DELETE action (implicit NO ACTION).
struct ForeignKeyWithoutAction;

impl LintRule for ForeignKeyWithoutAction {
    fn name(&self) -> &str {
        "fk-no-action"
    }
    fn description(&self) -> &str {
        "Foreign keys should have an explicit ON DELETE action"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for table in &schema.tables {
            for fk in &table.foreign_keys {
                if fk.on_delete == crate::schema::ForeignKeyAction::NoAction {
                    let fk_desc = fk.name.clone().unwrap_or_else(|| fk.columns.join(","));
                    warnings.push(LintWarning {
                        rule: self.name().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "foreign key '{}' on table '{}' has no explicit ON DELETE action (defaults to NO ACTION)",
                            fk_desc, table.name
                        ),
                        table: Some(table.name.clone()),
                        column: None,
                    });
                }
            }
        }
        warnings
    }
}

/// Rule 6: Naming convention violations.
struct NamingConvention {
    convention: String,
}

impl LintRule for NamingConvention {
    fn name(&self) -> &str {
        "naming-convention"
    }
    fn description(&self) -> &str {
        "Table and column names should follow the configured naming convention"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        let check_fn: fn(&str) -> bool = match self.convention.as_str() {
            "camelCase" => is_camel_case,
            _ => is_snake_case,
        };

        for table in &schema.tables {
            if !check_fn(&table.name) {
                warnings.push(LintWarning {
                    rule: self.name().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "table name '{}' does not follow {} convention",
                        table.name, self.convention
                    ),
                    table: Some(table.name.clone()),
                    column: None,
                });
            }
            for col in &table.columns {
                if !check_fn(&col.name) {
                    warnings.push(LintWarning {
                        rule: self.name().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "column name '{}.{}' does not follow {} convention",
                            table.name, col.name, self.convention
                        ),
                        table: Some(table.name.clone()),
                        column: Some(col.name.clone()),
                    });
                }
            }
        }
        warnings
    }
}

/// Rule 7: Duplicate indexes (same columns, same order).
struct DuplicateIndex;

impl LintRule for DuplicateIndex {
    fn name(&self) -> &str {
        "duplicate-index"
    }
    fn description(&self) -> &str {
        "Duplicate indexes waste storage and slow down writes"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for table in &schema.tables {
            for (i, idx_a) in table.indexes.iter().enumerate() {
                for idx_b in table.indexes.iter().skip(i + 1) {
                    if idx_a.columns == idx_b.columns {
                        let name_a = idx_a.name.as_deref().unwrap_or("unnamed");
                        let name_b = idx_b.name.as_deref().unwrap_or("unnamed");
                        warnings.push(LintWarning {
                            rule: self.name().to_string(),
                            severity: Severity::Warning,
                            message: format!(
                                "indexes '{}' and '{}' on table '{}' have the same columns",
                                name_a, name_b, table.name
                            ),
                            table: Some(table.name.clone()),
                            column: None,
                        });
                    }
                }
            }
        }
        warnings
    }
}

/// Rule 8: Enum with too many values.
struct EnumTooManyValues {
    threshold: usize,
}

impl LintRule for EnumTooManyValues {
    fn name(&self) -> &str {
        "enum-too-many-values"
    }
    fn description(&self) -> &str {
        "Enums with too many values may indicate a modeling problem"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for e in &schema.enums {
            if e.values.len() > self.threshold {
                warnings.push(LintWarning {
                    rule: self.name().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "enum '{}' has {} values (threshold: {}); consider using a lookup table",
                        e.name,
                        e.values.len(),
                        self.threshold
                    ),
                    table: None,
                    column: None,
                });
            }
        }
        warnings
    }
}

/// Rule 9: Missing created_at/updated_at timestamps (optional).
struct MissingTimestamps {
    enabled: bool,
}

impl LintRule for MissingTimestamps {
    fn name(&self) -> &str {
        "missing-timestamps"
    }
    fn description(&self) -> &str {
        "Tables should have created_at and updated_at timestamp columns"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        if !self.enabled {
            return Vec::new();
        }
        let mut warnings = Vec::new();
        for table in &schema.tables {
            let has_created = table.columns.iter().any(|c| c.name == "created_at");
            let has_updated = table.columns.iter().any(|c| c.name == "updated_at");
            if !has_created {
                warnings.push(LintWarning {
                    rule: self.name().to_string(),
                    severity: Severity::Info,
                    message: format!("table '{}' is missing a 'created_at' column", table.name),
                    table: Some(table.name.clone()),
                    column: None,
                });
            }
            if !has_updated {
                warnings.push(LintWarning {
                    rule: self.name().to_string(),
                    severity: Severity::Info,
                    message: format!("table '{}' is missing an 'updated_at' column", table.name),
                    table: Some(table.name.clone()),
                    column: None,
                });
            }
        }
        warnings
    }
}

/// Rule 10: Index on low-cardinality boolean column.
struct BooleanColumnIndex;

impl LintRule for BooleanColumnIndex {
    fn name(&self) -> &str {
        "boolean-index"
    }
    fn description(&self) -> &str {
        "Indexes on boolean columns are rarely useful due to low cardinality"
    }
    fn check(&self, schema: &Schema) -> Vec<LintWarning> {
        let mut warnings = Vec::new();
        for table in &schema.tables {
            let bool_cols: Vec<&str> = table
                .columns
                .iter()
                .filter(|c| c.data_type.is_boolean())
                .map(|c| c.name.as_str())
                .collect();

            for idx in &table.indexes {
                // Only flag single-column boolean indexes (composites with booleans
                // are often partial index patterns, which are fine)
                if idx.columns.len() == 1
                    && idx.predicate.is_none()
                    && bool_cols.contains(&idx.columns[0].as_str())
                {
                    let idx_name = idx.name.as_deref().unwrap_or("unnamed");
                    warnings.push(LintWarning {
                        rule: self.name().to_string(),
                        severity: Severity::Info,
                        message: format!(
                            "index '{}' on table '{}' indexes boolean column '{}'; consider a partial index instead",
                            idx_name, table.name, idx.columns[0]
                        ),
                        table: Some(table.name.clone()),
                        column: Some(idx.columns[0].clone()),
                    });
                }
            }
        }
        warnings
    }
}

// --- Helpers ---

/// Returns the set of column names that are covered by at least one index or the primary key.
pub fn table_indexed_columns_set(table: &Table) -> std::collections::HashSet<&str> {
    let mut cols = std::collections::HashSet::new();
    // Primary key columns are implicitly indexed
    if let Some(pk) = &table.primary_key {
        for c in &pk.columns {
            cols.insert(c.as_str());
        }
    }
    for idx in &table.indexes {
        for c in &idx.columns {
            cols.insert(c.as_str());
        }
    }
    cols
}

fn is_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !s.starts_with('_')
        && !s.ends_with('_')
        && !s.contains("__")
}

fn is_camel_case(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && !s.contains('_')
        && s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    fn default_config() -> LintConfig {
        LintConfig::default()
    }

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
    fn test_missing_primary_key() {
        let schema = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "missing-primary-key"));
    }

    #[test]
    fn test_no_warning_with_primary_key() {
        let schema = Schema {
            tables: vec![Table {
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string()],
                }),
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(!warnings.iter().any(|w| w.rule == "missing-primary-key"));
    }

    #[test]
    fn test_missing_fk_index() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("org_id", DataType::Integer)],
                foreign_keys: vec![ForeignKey {
                    name: Some("fk_org".to_string()),
                    columns: vec!["org_id".to_string()],
                    referenced_table: "orgs".to_string(),
                    referenced_schema: None,
                    referenced_columns: vec!["id".to_string()],
                    on_delete: ForeignKeyAction::Cascade,
                    on_update: ForeignKeyAction::NoAction,
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "missing-fk-index"));
    }

    #[test]
    fn test_no_fk_index_warning_when_indexed() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("org_id", DataType::Integer)],
                foreign_keys: vec![ForeignKey {
                    name: None,
                    columns: vec!["org_id".to_string()],
                    referenced_table: "orgs".to_string(),
                    referenced_schema: None,
                    referenced_columns: vec!["id".to_string()],
                    on_delete: ForeignKeyAction::Cascade,
                    on_update: ForeignKeyAction::NoAction,
                }],
                indexes: vec![Index {
                    name: Some("idx_org_id".to_string()),
                    columns: vec!["org_id".to_string()],
                    unique: false,
                    index_type: None,
                    predicate: None,
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(!warnings.iter().any(|w| w.rule == "missing-fk-index"));
    }

    #[test]
    fn test_nullable_unique() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![Column {
                    nullable: true,
                    ..make_column("email", DataType::Text)
                }],
                unique_constraints: vec![UniqueConstraint {
                    name: None,
                    columns: vec!["email".to_string()],
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "nullable-unique"));
    }

    #[test]
    fn test_unbounded_text() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("bio", DataType::Text)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "unbounded-text"));
    }

    #[test]
    fn test_fk_no_action() {
        let schema = Schema {
            tables: vec![Table {
                foreign_keys: vec![ForeignKey {
                    name: Some("fk_org".to_string()),
                    columns: vec!["org_id".to_string()],
                    referenced_table: "orgs".to_string(),
                    referenced_schema: None,
                    referenced_columns: vec!["id".to_string()],
                    on_delete: ForeignKeyAction::NoAction,
                    on_update: ForeignKeyAction::NoAction,
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "fk-no-action"));
    }

    #[test]
    fn test_naming_convention_snake_case() {
        let schema = Schema {
            tables: vec![Table::new("UserAccounts")],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "naming-convention"));
    }

    #[test]
    fn test_naming_convention_passes() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("first_name", DataType::Text)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(!warnings.iter().any(|w| w.rule == "naming-convention"));
    }

    #[test]
    fn test_duplicate_index() {
        let idx = Index {
            name: Some("idx_a".to_string()),
            columns: vec!["email".to_string()],
            unique: false,
            index_type: None,
            predicate: None,
        };
        let idx2 = Index {
            name: Some("idx_b".to_string()),
            ..idx.clone()
        };
        let schema = Schema {
            tables: vec![Table {
                indexes: vec![idx, idx2],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "duplicate-index"));
    }

    #[test]
    fn test_enum_too_many_values() {
        let schema = Schema {
            enums: vec![EnumType {
                name: "big_enum".to_string(),
                values: (0..25).map(|i| format!("val_{i}")).collect(),
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "enum-too-many-values"));
    }

    #[test]
    fn test_missing_timestamps_disabled_by_default() {
        let schema = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(!warnings.iter().any(|w| w.rule == "missing-timestamps"));
    }

    #[test]
    fn test_missing_timestamps_enabled() {
        let schema = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let config = LintConfig {
            require_timestamps: true,
            ..default_config()
        };
        let warnings = lint(&schema, &config);
        let ts_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| w.rule == "missing-timestamps")
            .collect();
        assert_eq!(ts_warnings.len(), 2); // missing both created_at and updated_at
    }

    #[test]
    fn test_boolean_index() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("is_active", DataType::Boolean)],
                indexes: vec![Index {
                    name: Some("idx_active".to_string()),
                    columns: vec!["is_active".to_string()],
                    unique: false,
                    index_type: None,
                    predicate: None,
                }],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let warnings = lint(&schema, &default_config());
        assert!(warnings.iter().any(|w| w.rule == "boolean-index"));
    }

    #[test]
    fn test_disabled_rule() {
        let schema = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let config = LintConfig {
            disabled_rules: vec!["missing-primary-key".to_string()],
            ..default_config()
        };
        let warnings = lint(&schema, &config);
        assert!(!warnings.iter().any(|w| w.rule == "missing-primary-key"));
    }

    #[test]
    fn test_is_snake_case() {
        assert!(is_snake_case("user_accounts"));
        assert!(is_snake_case("users"));
        assert!(is_snake_case("id"));
        assert!(!is_snake_case("UserAccounts"));
        assert!(!is_snake_case("user__accounts"));
        assert!(!is_snake_case("_users"));
        assert!(!is_snake_case(""));
    }

    #[test]
    fn test_is_camel_case() {
        assert!(is_camel_case("userAccounts"));
        assert!(is_camel_case("users"));
        assert!(!is_camel_case("UserAccounts"));
        assert!(!is_camel_case("user_accounts"));
        assert!(!is_camel_case(""));
    }
}
