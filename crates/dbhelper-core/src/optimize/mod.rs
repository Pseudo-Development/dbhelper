use crate::schema::{DataType, ForeignKeyAction, Schema, Table};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Suggestion {
    pub rule: String,
    pub message: String,
    pub table: Option<String>,
    pub column: Option<String>,
}

/// Analyze a schema and return optimization suggestions.
pub fn analyze(schema: &Schema) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    for table in &schema.tables {
        suggest_composite_fk_indexes(table, &mut suggestions);
        suggest_varchar_sizing(table, &mut suggestions);
        suggest_partial_indexes(table, &mut suggestions);
        detect_redundant_indexes(table, &mut suggestions);
        suggest_type_downgrades(table, &mut suggestions);
        suggest_missing_join_indexes(table, &mut suggestions);
        suggest_denormalization(table, schema, &mut suggestions);
    }

    suggestions
}

/// Rule 1: Suggest composite indexes combining FK columns with status/type columns.
fn suggest_composite_fk_indexes(table: &Table, suggestions: &mut Vec<Suggestion>) {
    let status_cols: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| {
            matches!(c.data_type, DataType::Enum { .. } | DataType::Boolean)
                || c.name.ends_with("_status")
                || c.name.ends_with("_type")
                || c.name == "status"
                || c.name == "type"
        })
        .map(|c| c.name.as_str())
        .collect();

    if status_cols.is_empty() {
        return;
    }

    for fk in &table.foreign_keys {
        for fk_col in &fk.columns {
            // Check if there's already a composite index with this FK + a status column
            let has_composite = table.indexes.iter().any(|idx| {
                idx.columns.len() >= 2
                    && idx.columns.contains(fk_col)
                    && idx
                        .columns
                        .iter()
                        .any(|c| status_cols.contains(&c.as_str()))
            });

            if !has_composite {
                for status_col in &status_cols {
                    suggestions.push(Suggestion {
                        rule: "composite-fk-index".to_string(),
                        message: format!(
                            "consider a composite index on ({}, {}) for filtered FK lookups",
                            fk_col, status_col
                        ),
                        table: Some(table.name.clone()),
                        column: Some(fk_col.clone()),
                    });
                }
            }
        }
    }
}

/// Rule 2: Identify potentially oversized varchar columns.
fn suggest_varchar_sizing(table: &Table, suggestions: &mut Vec<Suggestion>) {
    for col in &table.columns {
        if let DataType::Varchar(Some(len)) = col.data_type {
            if len >= 1000 {
                suggestions.push(Suggestion {
                    rule: "oversized-varchar".to_string(),
                    message: format!(
                        "column '{}.{}' is varchar({}); consider using text type instead for large content",
                        table.name, col.name, len
                    ),
                    table: Some(table.name.clone()),
                    column: Some(col.name.clone()),
                });
            }
        }
    }
}

/// Rule 3: Suggest partial indexes for tables with boolean/status columns.
fn suggest_partial_indexes(table: &Table, suggestions: &mut Vec<Suggestion>) {
    let has_partial = table.indexes.iter().any(|idx| idx.predicate.is_some());
    if has_partial {
        return;
    }

    for col in &table.columns {
        if col.data_type.is_boolean()
            && (col.name.starts_with("is_") || col.name.starts_with("has_"))
        {
            // Check if there's a regular index on this column
            let has_regular_index = table
                .indexes
                .iter()
                .any(|idx| idx.columns.len() == 1 && idx.columns[0] == col.name);

            if has_regular_index {
                suggestions.push(Suggestion {
                    rule: "partial-index".to_string(),
                    message: format!(
                        "column '{}.{}' has a full index on a boolean; consider a partial index WHERE {} = false",
                        table.name, col.name, col.name
                    ),
                    table: Some(table.name.clone()),
                    column: Some(col.name.clone()),
                });
            }
        }
    }
}

/// Rule 4: Detect redundant indexes (one index is a prefix of another).
fn detect_redundant_indexes(table: &Table, suggestions: &mut Vec<Suggestion>) {
    for (i, idx_a) in table.indexes.iter().enumerate() {
        for idx_b in table.indexes.iter().skip(i + 1) {
            if is_prefix_of(&idx_a.columns, &idx_b.columns) {
                let name_a = idx_a.name.as_deref().unwrap_or("unnamed");
                let name_b = idx_b.name.as_deref().unwrap_or("unnamed");
                suggestions.push(Suggestion {
                    rule: "redundant-index".to_string(),
                    message: format!(
                        "index '{}' on ({}) is a prefix of '{}' on ({}); the shorter index may be redundant",
                        name_a,
                        idx_a.columns.join(", "),
                        name_b,
                        idx_b.columns.join(", ")
                    ),
                    table: Some(table.name.clone()),
                    column: None,
                });
            } else if is_prefix_of(&idx_b.columns, &idx_a.columns) {
                let name_a = idx_a.name.as_deref().unwrap_or("unnamed");
                let name_b = idx_b.name.as_deref().unwrap_or("unnamed");
                suggestions.push(Suggestion {
                    rule: "redundant-index".to_string(),
                    message: format!(
                        "index '{}' on ({}) is a prefix of '{}' on ({}); the shorter index may be redundant",
                        name_b,
                        idx_b.columns.join(", "),
                        name_a,
                        idx_a.columns.join(", ")
                    ),
                    table: Some(table.name.clone()),
                    column: None,
                });
            }
        }
    }
}

/// Rule 5: Suggest data type downgrades (e.g. bigint -> integer when constraints allow).
fn suggest_type_downgrades(table: &Table, suggestions: &mut Vec<Suggestion>) {
    for col in &table.columns {
        // If using bigint for a non-PK, non-FK column, suggest integer
        if col.data_type == DataType::BigInt && !col.auto_increment {
            let is_pk = table
                .primary_key
                .as_ref()
                .is_some_and(|pk| pk.columns.contains(&col.name));
            let is_fk = table
                .foreign_keys
                .iter()
                .any(|fk| fk.columns.contains(&col.name));
            if !is_pk && !is_fk {
                suggestions.push(Suggestion {
                    rule: "type-downgrade".to_string(),
                    message: format!(
                        "column '{}.{}' uses bigint but is not a primary or foreign key; consider integer if values fit",
                        table.name, col.name
                    ),
                    table: Some(table.name.clone()),
                    column: Some(col.name.clone()),
                });
            }
        }
    }
}

/// Rule 6: Flag missing join indexes (FK columns without indexes for N+1 query patterns).
fn suggest_missing_join_indexes(table: &Table, suggestions: &mut Vec<Suggestion>) {
    let indexed_cols = crate::lint::table_indexed_columns_set(table);

    for fk in &table.foreign_keys {
        // Only look at multi-column FKs or FKs with cascade deletes
        // (these are frequently used in joins)
        if fk.on_delete == ForeignKeyAction::Cascade || fk.columns.len() > 1 {
            for col in &fk.columns {
                if !indexed_cols.contains(col.as_str()) {
                    suggestions.push(Suggestion {
                        rule: "missing-join-index".to_string(),
                        message: format!(
                            "FK column '{}.{}' (references '{}') lacks an index; cascading deletes and joins will table-scan",
                            table.name, col, fk.referenced_table
                        ),
                        table: Some(table.name.clone()),
                        column: Some(col.clone()),
                    });
                }
            }
        }
    }
}

/// Rule 7: Identify potential denormalization opportunities.
fn suggest_denormalization(table: &Table, schema: &Schema, suggestions: &mut Vec<Suggestion>) {
    // If a table has many FK columns (3+), it might benefit from denormalization
    if table.foreign_keys.len() >= 3 {
        // Check if any referenced tables are small lookup tables (few columns, PK only)
        for fk in &table.foreign_keys {
            if let Some(ref_table) = schema.tables.iter().find(|t| t.name == fk.referenced_table) {
                if ref_table.columns.len() <= 3 && ref_table.foreign_keys.is_empty() {
                    suggestions.push(Suggestion {
                        rule: "denormalization".to_string(),
                        message: format!(
                            "table '{}' references small lookup table '{}'; consider denormalizing frequently-accessed columns",
                            table.name, ref_table.name
                        ),
                        table: Some(table.name.clone()),
                        column: None,
                    });
                }
            }
        }
    }
}

fn is_prefix_of(shorter: &[String], longer: &[String]) -> bool {
    shorter.len() < longer.len() && longer.starts_with(shorter)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn make_fk(col: &str, ref_table: &str, on_delete: ForeignKeyAction) -> ForeignKey {
        ForeignKey {
            name: None,
            columns: vec![col.to_string()],
            referenced_table: ref_table.to_string(),
            referenced_schema: None,
            referenced_columns: vec!["id".to_string()],
            on_delete,
            on_update: ForeignKeyAction::NoAction,
        }
    }

    #[test]
    fn test_composite_fk_index_suggestion() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![
                    make_column("user_id", DataType::Integer),
                    make_column(
                        "status",
                        DataType::Enum {
                            name: Some("status".to_string()),
                            values: vec![],
                        },
                    ),
                ],
                foreign_keys: vec![make_fk("user_id", "users", ForeignKeyAction::Cascade)],
                ..Table::new("orders")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "composite-fk-index"));
    }

    #[test]
    fn test_oversized_varchar() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("description", DataType::Varchar(Some(5000)))],
                ..Table::new("products")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "oversized-varchar"));
    }

    #[test]
    fn test_no_oversized_varchar_for_normal() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("name", DataType::Varchar(Some(255)))],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(!suggestions.iter().any(|s| s.rule == "oversized-varchar"));
    }

    #[test]
    fn test_partial_index_suggestion() {
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
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "partial-index"));
    }

    #[test]
    fn test_redundant_index() {
        let schema = Schema {
            tables: vec![Table {
                indexes: vec![
                    Index {
                        name: Some("idx_a".to_string()),
                        columns: vec!["user_id".to_string()],
                        unique: false,
                        index_type: None,
                        predicate: None,
                    },
                    Index {
                        name: Some("idx_b".to_string()),
                        columns: vec!["user_id".to_string(), "created_at".to_string()],
                        unique: false,
                        index_type: None,
                        predicate: None,
                    },
                ],
                ..Table::new("orders")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "redundant-index"));
    }

    #[test]
    fn test_type_downgrade_bigint() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("sort_order", DataType::BigInt)],
                ..Table::new("items")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "type-downgrade"));
    }

    #[test]
    fn test_no_type_downgrade_for_pk() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![Column {
                    auto_increment: true,
                    ..make_column("id", DataType::BigInt)
                }],
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string()],
                }),
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(!suggestions.iter().any(|s| s.rule == "type-downgrade"));
    }

    #[test]
    fn test_missing_join_index() {
        let schema = Schema {
            tables: vec![Table {
                columns: vec![make_column("user_id", DataType::Integer)],
                foreign_keys: vec![make_fk("user_id", "users", ForeignKeyAction::Cascade)],
                ..Table::new("orders")
            }],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "missing-join-index"));
    }

    #[test]
    fn test_denormalization_suggestion() {
        let tags = Table {
            columns: vec![
                make_column("id", DataType::Integer),
                make_column("name", DataType::Varchar(Some(50))),
            ],
            primary_key: Some(PrimaryKey {
                name: None,
                columns: vec!["id".to_string()],
            }),
            ..Table::new("tags")
        };
        let categories = Table {
            columns: vec![
                make_column("id", DataType::Integer),
                make_column("name", DataType::Varchar(Some(100))),
            ],
            ..Table::new("categories")
        };
        let statuses = Table {
            columns: vec![
                make_column("id", DataType::Integer),
                make_column("label", DataType::Varchar(Some(50))),
            ],
            ..Table::new("statuses")
        };
        let products = Table {
            columns: vec![
                make_column("id", DataType::Integer),
                make_column("tag_id", DataType::Integer),
                make_column("cat_id", DataType::Integer),
                make_column("status_id", DataType::Integer),
            ],
            foreign_keys: vec![
                make_fk("tag_id", "tags", ForeignKeyAction::NoAction),
                make_fk("cat_id", "categories", ForeignKeyAction::NoAction),
                make_fk("status_id", "statuses", ForeignKeyAction::NoAction),
            ],
            ..Table::new("products")
        };
        let schema = Schema {
            tables: vec![tags, categories, statuses, products],
            ..Schema::new("public")
        };
        let suggestions = analyze(&schema);
        assert!(suggestions.iter().any(|s| s.rule == "denormalization"));
    }
}
