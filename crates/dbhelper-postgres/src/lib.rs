use dbhelper_core::schema::Schema;

/// Introspect a Postgres database and return its schema.
pub async fn introspect(_connection_url: &str) -> Result<Schema, sqlx::Error> {
    // TODO: query information_schema to build Schema
    Ok(Schema {
        tables: Vec::new(),
    })
}
