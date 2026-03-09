use std::path::PathBuf;

/// Errors from parsing SQL migration files.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("failed to read migration file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("SQL parse error in {file} at line {line}: {message}")]
    Sql {
        file: PathBuf,
        line: usize,
        message: String,
    },

    #[error("unsupported SQL statement in {file}: {statement}")]
    UnsupportedStatement { file: PathBuf, statement: String },

    #[error("migration directory not found: {0}")]
    DirectoryNotFound(PathBuf),

    #[error("no migration files found in {0}")]
    NoMigrations(PathBuf),
}

/// Errors from the schema diff engine.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("failed to read previous state from {path}: {source}")]
    ReadState {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse previous state from {path}: {source}")]
    ParseState {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to write state to {path}: {source}")]
    WriteState {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Errors from the lint engine.
#[derive(Debug, thiserror::Error)]
pub enum LintError {
    #[error("unknown lint rule: {0}")]
    UnknownRule(String),

    #[error("lint configuration error: {0}")]
    Config(String),
}
