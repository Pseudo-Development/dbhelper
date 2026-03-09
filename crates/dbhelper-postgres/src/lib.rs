use dbhelper_core::schema::Schema;

/// Errors from Postgres introspection.
#[derive(Debug, thiserror::Error)]
pub enum PgError {
    #[error("postgres connection error: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("failed to introspect schema '{schema}': {message}")]
    Introspection { schema: String, message: String },

    #[error("unsupported postgres type: {0}")]
    UnsupportedType(String),
}

/// Introspect a Postgres database and return its schema.
pub async fn introspect(_connection_url: &str) -> Result<Schema, PgError> {
    // TODO: query information_schema to build Schema
    Ok(Schema::new("public"))
}
