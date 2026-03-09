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
