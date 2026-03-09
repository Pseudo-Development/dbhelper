use crate::schema::Schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintWarning {
    pub rule: String,
    pub message: String,
    pub table: Option<String>,
    pub column: Option<String>,
}

/// Run all lint rules against a schema.
pub fn lint(_schema: &Schema) -> Vec<LintWarning> {
    // TODO: implement lint rules
    Vec::new()
}
