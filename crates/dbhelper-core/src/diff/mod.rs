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
    AddColumn { table: String, column: String },
    DropColumn { table: String, column: String },
    AlterColumn { table: String, column: String },
    AddIndex { table: String, index: String },
    DropIndex { table: String, index: String },
}

/// Compute the diff between two schemas.
pub fn diff(_from: &Schema, _to: &Schema) -> SchemaDiff {
    // TODO: implement schema diffing
    SchemaDiff {
        changes: Vec::new(),
    }
}
