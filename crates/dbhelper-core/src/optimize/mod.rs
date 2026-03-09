use crate::schema::Schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub rule: String,
    pub message: String,
    pub table: Option<String>,
}

/// Analyze a schema and return optimization suggestions.
pub fn analyze(_schema: &Schema) -> Vec<Suggestion> {
    // TODO: implement optimization analysis
    Vec::new()
}
