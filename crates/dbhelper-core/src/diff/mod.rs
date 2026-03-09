use crate::schema::Schema;
use serde::{Deserialize, Serialize};

/// Represents the difference between two schemas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    },
}

/// Compute the diff between two schemas.
pub fn diff(_from: &Schema, _to: &Schema) -> SchemaDiff {
    // TODO: implement schema diffing
    SchemaDiff {
        changes: Vec::new(),
    }
}
