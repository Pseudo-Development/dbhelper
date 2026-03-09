use std::collections::{HashMap, HashSet};

use crate::schema::{
    CheckConstraint, Column, EnumType, ForeignKey, Index, Schema, Table, UniqueConstraint,
};
use serde::{Deserialize, Serialize};

/// Represents the difference between two schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub changes: Vec<Change>,
}

impl SchemaDiff {
    /// Returns true if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Change {
    AddTable(String),
    DropTable(String),
    AddColumn {
        table: String,
        column: String,
    },
    DropColumn {
        table: String,
        column: String,
    },
    AlterColumn {
        table: String,
        column: String,
        description: String,
    },
    AddIndex {
        table: String,
        index: String,
    },
    DropIndex {
        table: String,
        index: String,
    },
    AddForeignKey {
        table: String,
        name: String,
    },
    DropForeignKey {
        table: String,
        name: String,
    },
    AddCheckConstraint {
        table: String,
        name: String,
    },
    DropCheckConstraint {
        table: String,
        name: String,
    },
    AddUniqueConstraint {
        table: String,
        name: String,
    },
    DropUniqueConstraint {
        table: String,
        name: String,
    },
    AddEnum {
        name: String,
    },
    DropEnum {
        name: String,
    },
    AlterEnum {
        name: String,
        description: String,
    },
    ChangePrimaryKey {
        table: String,
        /// Name of the old primary key constraint (if known), for DROP.
        old_pk_name: Option<String>,
    },
}

/// Compute the diff between two schemas.
pub fn diff(from: &Schema, to: &Schema) -> SchemaDiff {
    let mut changes = Vec::new();

    diff_enums(&from.enums, &to.enums, &mut changes);
    diff_tables(&from.tables, &to.tables, &mut changes);

    SchemaDiff { changes }
}

/// Detect cross-source conflicts: tables defined by multiple sources.
pub fn detect_conflicts(schemas: &[(&str, &Schema)]) -> Vec<String> {
    let mut table_owners: HashMap<&str, Vec<&str>> = HashMap::new();
    for (source, schema) in schemas {
        for table in &schema.tables {
            table_owners.entry(&table.name).or_default().push(source);
        }
    }

    let mut conflicts = Vec::new();
    for (table, owners) in &table_owners {
        if owners.len() > 1 {
            conflicts.push(format!(
                "table '{}' is defined by multiple sources: {}",
                table,
                owners.join(", ")
            ));
        }
    }
    conflicts.sort();
    conflicts
}

fn diff_enums(from: &[EnumType], to: &[EnumType], changes: &mut Vec<Change>) {
    let from_map: HashMap<&str, &EnumType> = from.iter().map(|e| (e.name.as_str(), e)).collect();
    let to_map: HashMap<&str, &EnumType> = to.iter().map(|e| (e.name.as_str(), e)).collect();

    for name in sorted_keys(&from_map) {
        if !to_map.contains_key(name) {
            changes.push(Change::DropEnum {
                name: name.to_string(),
            });
        }
    }

    for name in sorted_keys(&to_map) {
        match from_map.get(name) {
            None => {
                changes.push(Change::AddEnum {
                    name: name.to_string(),
                });
            }
            Some(old) => {
                let new = to_map[name];
                if old.values != new.values {
                    let added: Vec<&String> = new
                        .values
                        .iter()
                        .filter(|v| !old.values.contains(v))
                        .collect();
                    let removed: Vec<&String> = old
                        .values
                        .iter()
                        .filter(|v| !new.values.contains(v))
                        .collect();

                    let mut parts = Vec::new();
                    if !added.is_empty() {
                        parts.push(format!(
                            "added values: {}",
                            added
                                .iter()
                                .map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    if !removed.is_empty() {
                        parts.push(format!(
                            "removed values: {}",
                            removed
                                .iter()
                                .map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }

                    changes.push(Change::AlterEnum {
                        name: name.to_string(),
                        description: parts.join("; "),
                    });
                }
            }
        }
    }
}

fn diff_tables(from: &[Table], to: &[Table], changes: &mut Vec<Change>) {
    let from_map: HashMap<&str, &Table> = from.iter().map(|t| (t.name.as_str(), t)).collect();
    let to_map: HashMap<&str, &Table> = to.iter().map(|t| (t.name.as_str(), t)).collect();

    for name in sorted_keys(&from_map) {
        if !to_map.contains_key(name) {
            changes.push(Change::DropTable(name.to_string()));
        }
    }

    for name in sorted_keys(&to_map) {
        match from_map.get(name) {
            None => {
                changes.push(Change::AddTable(name.to_string()));
            }
            Some(old) => {
                let new = to_map[name];
                diff_table(old, new, changes);
            }
        }
    }
}

fn diff_table(from: &Table, to: &Table, changes: &mut Vec<Change>) {
    let table = &to.name;

    diff_columns(table, &from.columns, &to.columns, changes);
    diff_primary_key(table, from, to, changes);
    diff_indexes(table, &from.indexes, &to.indexes, changes);
    diff_foreign_keys(table, &from.foreign_keys, &to.foreign_keys, changes);
    diff_check_constraints(
        table,
        &from.check_constraints,
        &to.check_constraints,
        changes,
    );
    diff_unique_constraints(
        table,
        &from.unique_constraints,
        &to.unique_constraints,
        changes,
    );
}

fn diff_columns(table: &str, from: &[Column], to: &[Column], changes: &mut Vec<Change>) {
    let from_map: HashMap<&str, &Column> = from.iter().map(|c| (c.name.as_str(), c)).collect();
    let to_map: HashMap<&str, &Column> = to.iter().map(|c| (c.name.as_str(), c)).collect();

    for name in sorted_keys(&from_map) {
        if !to_map.contains_key(name) {
            changes.push(Change::DropColumn {
                table: table.to_string(),
                column: name.to_string(),
            });
        }
    }

    for name in sorted_keys(&to_map) {
        match from_map.get(name) {
            None => {
                changes.push(Change::AddColumn {
                    table: table.to_string(),
                    column: name.to_string(),
                });
            }
            Some(old) => {
                let new = to_map[name];
                let mut diffs = Vec::new();

                if old.data_type != new.data_type {
                    diffs.push(format!("type: {} -> {}", old.data_type, new.data_type));
                }
                if old.nullable != new.nullable {
                    diffs.push(format!("nullable: {} -> {}", old.nullable, new.nullable));
                }
                if old.default != new.default {
                    diffs.push(format!(
                        "default: {} -> {}",
                        format_default(&old.default),
                        format_default(&new.default)
                    ));
                }
                if old.auto_increment != new.auto_increment {
                    diffs.push(format!(
                        "auto_increment: {} -> {}",
                        old.auto_increment, new.auto_increment
                    ));
                }
                if old.unsigned != new.unsigned {
                    diffs.push(format!("unsigned: {} -> {}", old.unsigned, new.unsigned));
                }

                if !diffs.is_empty() {
                    changes.push(Change::AlterColumn {
                        table: table.to_string(),
                        column: name.to_string(),
                        description: diffs.join(", "),
                    });
                }
            }
        }
    }
}

fn diff_primary_key(table: &str, from: &Table, to: &Table, changes: &mut Vec<Change>) {
    if from.primary_key != to.primary_key {
        let old_pk_name = from.primary_key.as_ref().and_then(|pk| pk.name.clone());
        changes.push(Change::ChangePrimaryKey {
            table: table.to_string(),
            old_pk_name,
        });
    }
}

fn diff_indexes(table: &str, from: &[Index], to: &[Index], changes: &mut Vec<Change>) {
    let from_set: HashSet<&Index> = from.iter().collect();
    let to_set: HashSet<&Index> = to.iter().collect();

    for idx in from {
        if !to_set.contains(idx) {
            changes.push(Change::DropIndex {
                table: table.to_string(),
                index: index_name(idx),
            });
        }
    }
    for idx in to {
        if !from_set.contains(idx) {
            changes.push(Change::AddIndex {
                table: table.to_string(),
                index: index_name(idx),
            });
        }
    }
}

fn diff_foreign_keys(
    table: &str,
    from: &[ForeignKey],
    to: &[ForeignKey],
    changes: &mut Vec<Change>,
) {
    let from_set: HashSet<&ForeignKey> = from.iter().collect();
    let to_set: HashSet<&ForeignKey> = to.iter().collect();

    for fk in from {
        if !to_set.contains(fk) {
            changes.push(Change::DropForeignKey {
                table: table.to_string(),
                name: fk_name(fk),
            });
        }
    }
    for fk in to {
        if !from_set.contains(fk) {
            changes.push(Change::AddForeignKey {
                table: table.to_string(),
                name: fk_name(fk),
            });
        }
    }
}

fn diff_check_constraints(
    table: &str,
    from: &[CheckConstraint],
    to: &[CheckConstraint],
    changes: &mut Vec<Change>,
) {
    let from_set: HashSet<&CheckConstraint> = from.iter().collect();
    let to_set: HashSet<&CheckConstraint> = to.iter().collect();

    for cc in from {
        if !to_set.contains(cc) {
            changes.push(Change::DropCheckConstraint {
                table: table.to_string(),
                name: cc.name.clone().unwrap_or_else(|| cc.expression.clone()),
            });
        }
    }
    for cc in to {
        if !from_set.contains(cc) {
            changes.push(Change::AddCheckConstraint {
                table: table.to_string(),
                name: cc.name.clone().unwrap_or_else(|| cc.expression.clone()),
            });
        }
    }
}

fn diff_unique_constraints(
    table: &str,
    from: &[UniqueConstraint],
    to: &[UniqueConstraint],
    changes: &mut Vec<Change>,
) {
    let from_set: HashSet<&UniqueConstraint> = from.iter().collect();
    let to_set: HashSet<&UniqueConstraint> = to.iter().collect();

    for uc in from {
        if !to_set.contains(uc) {
            changes.push(Change::DropUniqueConstraint {
                table: table.to_string(),
                name: uc.name.clone().unwrap_or_else(|| uc.columns.join(",")),
            });
        }
    }
    for uc in to {
        if !from_set.contains(uc) {
            changes.push(Change::AddUniqueConstraint {
                table: table.to_string(),
                name: uc.name.clone().unwrap_or_else(|| uc.columns.join(",")),
            });
        }
    }
}

fn format_default(d: &Option<crate::schema::ColumnDefault>) -> String {
    match d {
        None => "none".to_string(),
        Some(crate::schema::ColumnDefault::Literal(v)) => format!("'{v}'"),
        Some(crate::schema::ColumnDefault::Expression(e)) => e.clone(),
    }
}

fn index_name(idx: &Index) -> String {
    idx.name.clone().unwrap_or_else(|| idx.columns.join(","))
}

fn fk_name(fk: &ForeignKey) -> String {
    fk.name.clone().unwrap_or_else(|| {
        format!(
            "({}) -> {}({})",
            fk.columns.join(","),
            fk.referenced_table,
            fk.referenced_columns.join(","),
        )
    })
}

fn sorted_keys<'a, V>(map: &HashMap<&'a str, V>) -> Vec<&'a str> {
    let mut keys: Vec<&str> = map.keys().copied().collect();
    keys.sort();
    keys
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

    #[test]
    fn test_empty_schemas_no_diff() {
        let a = Schema::new("public");
        let b = Schema::new("public");
        let result = diff(&a, &b);
        assert!(result.is_empty());
    }

    #[test]
    fn test_add_table() {
        let from = Schema::new("public");
        let to = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(result.changes, vec![Change::AddTable("users".to_string())]);
    }

    #[test]
    fn test_drop_table() {
        let from = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let to = Schema::new("public");
        let result = diff(&from, &to);
        assert_eq!(result.changes, vec![Change::DropTable("users".to_string())]);
    }

    #[test]
    fn test_add_column() {
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
        let result = diff(&from, &to);
        assert_eq!(
            result.changes,
            vec![Change::AddColumn {
                table: "users".to_string(),
                column: "email".to_string(),
            }]
        );
    }

    #[test]
    fn test_alter_column_type() {
        let from = Schema {
            tables: vec![Table {
                columns: vec![make_column("age", DataType::Integer)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                columns: vec![make_column("age", DataType::BigInt)],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(result.changes.len(), 1);
        match &result.changes[0] {
            Change::AlterColumn {
                table,
                column,
                description,
            } => {
                assert_eq!(table, "users");
                assert_eq!(column, "age");
                assert!(description.contains("integer -> bigint"));
            }
            other => panic!("expected AlterColumn, got {other:?}"),
        }
    }

    #[test]
    fn test_add_drop_enum() {
        let from = Schema::new("public");
        let to = Schema {
            enums: vec![EnumType {
                name: "status".to_string(),
                values: vec!["active".to_string(), "inactive".to_string()],
            }],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(
            result.changes,
            vec![Change::AddEnum {
                name: "status".to_string()
            }]
        );

        let result2 = diff(&to, &from);
        assert_eq!(
            result2.changes,
            vec![Change::DropEnum {
                name: "status".to_string()
            }]
        );
    }

    #[test]
    fn test_alter_enum_values() {
        let from = Schema {
            enums: vec![EnumType {
                name: "role".to_string(),
                values: vec!["admin".to_string(), "user".to_string()],
            }],
            ..Schema::new("public")
        };
        let to = Schema {
            enums: vec![EnumType {
                name: "role".to_string(),
                values: vec!["admin".to_string(), "editor".to_string()],
            }],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(result.changes.len(), 1);
        match &result.changes[0] {
            Change::AlterEnum { name, description } => {
                assert_eq!(name, "role");
                assert!(description.contains("editor"));
                assert!(description.contains("user"));
            }
            other => panic!("expected AlterEnum, got {other:?}"),
        }
    }

    #[test]
    fn test_add_index() {
        let idx = Index {
            name: Some("idx_email".to_string()),
            columns: vec!["email".to_string()],
            unique: true,
            index_type: None,
            predicate: None,
        };
        let from = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                indexes: vec![idx],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(
            result.changes,
            vec![Change::AddIndex {
                table: "users".to_string(),
                index: "idx_email".to_string(),
            }]
        );
    }

    #[test]
    fn test_add_foreign_key() {
        let fk = ForeignKey {
            name: Some("fk_org".to_string()),
            columns: vec!["org_id".to_string()],
            referenced_table: "organizations".to_string(),
            referenced_schema: None,
            referenced_columns: vec!["id".to_string()],
            on_delete: ForeignKeyAction::Cascade,
            on_update: ForeignKeyAction::NoAction,
        };
        let from = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                foreign_keys: vec![fk],
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(
            result.changes,
            vec![Change::AddForeignKey {
                table: "users".to_string(),
                name: "fk_org".to_string(),
            }]
        );
    }

    #[test]
    fn test_change_primary_key() {
        let from = Schema {
            tables: vec![Table {
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string()],
                }),
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let to = Schema {
            tables: vec![Table {
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string(), "tenant_id".to_string()],
                }),
                ..Table::new("users")
            }],
            ..Schema::new("public")
        };
        let result = diff(&from, &to);
        assert_eq!(
            result.changes,
            vec![Change::ChangePrimaryKey {
                table: "users".to_string(),
                old_pk_name: None,
            }]
        );
    }

    #[test]
    fn test_detect_conflicts() {
        let s1 = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let s2 = Schema {
            tables: vec![Table::new("users"), Table::new("orders")],
            ..Schema::new("public")
        };
        let conflicts = detect_conflicts(&[("drizzle", &s1), ("alembic", &s2)]);
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].contains("users"));
        assert!(conflicts[0].contains("drizzle"));
        assert!(conflicts[0].contains("alembic"));
    }

    #[test]
    fn test_no_conflicts() {
        let s1 = Schema {
            tables: vec![Table::new("users")],
            ..Schema::new("public")
        };
        let s2 = Schema {
            tables: vec![Table::new("orders")],
            ..Schema::new("public")
        };
        let conflicts = detect_conflicts(&[("drizzle", &s1), ("alembic", &s2)]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_diff_serialization_roundtrip() {
        let d = SchemaDiff {
            changes: vec![
                Change::AddTable("users".to_string()),
                Change::AddColumn {
                    table: "users".to_string(),
                    column: "email".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: SchemaDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back.changes.len(), 2);
    }
}
