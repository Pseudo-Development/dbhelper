use dbhelper_core::schema::Schema;

/// Errors from MySQL introspection.
#[derive(Debug, thiserror::Error)]
pub enum MysqlError {
    #[error("mysql connection error: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("failed to introspect database '{database}': {message}")]
    Introspection { database: String, message: String },

    #[error("unsupported mysql type: {0}")]
    UnsupportedType(String),
}

/// Introspect a MySQL database and return its schema.
pub async fn introspect(_connection_url: &str) -> Result<Schema, MysqlError> {
    // TODO: query information_schema to build Schema
    Ok(Schema::new("default"))
}
