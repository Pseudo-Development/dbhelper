/// Errors from container management.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("failed to start {kind} container: {message}")]
    StartFailed { kind: String, message: String },

    #[error("container health check failed after {attempts} attempts: {message}")]
    HealthCheckFailed { attempts: u32, message: String },

    #[error("failed to apply migrations: {0}")]
    MigrationFailed(String),

    #[error("docker error: {0}")]
    Docker(String),
}

/// Database container type to spin up.
pub enum DatabaseKind {
    Postgres,
    Mysql,
}

/// Manages ephemeral database containers for testing and migration replay.
pub struct ContainerManager;

impl ContainerManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContainerManager {
    fn default() -> Self {
        Self::new()
    }
}
