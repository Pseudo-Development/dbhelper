use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use dbhelper_core::config::{Config, ConfigError, DatabaseConfig};
use dbhelper_core::diff::{self, Change, SchemaDiff};
use dbhelper_core::lint::{self, LintWarning, Severity};
use dbhelper_core::optimize::{self, Suggestion};
use dbhelper_core::parser;
use dbhelper_core::schema::Schema;

/// CLI-level errors wrapping all sub-crate errors.
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Config(#[from] ConfigError),

    #[error("{0}")]
    Parse(#[from] dbhelper_core::error::ParseError),

    #[error("{0}")]
    Diff(#[from] dbhelper_core::error::DiffError),

    #[error("{0}")]
    Lint(#[from] dbhelper_core::error::LintError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Parser)]
#[command(
    name = "dbhelper",
    about = "Database linter, diff, and optimization tool"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compare newly generated schema state against the last known state
    Diff {
        /// Path to dbhelper config file (toml)
        #[arg(default_value = "dbhelper.toml")]
        config: PathBuf,

        /// Only diff a specific database by name
        #[arg(long)]
        database: Option<String>,

        /// Output format
        #[arg(long, default_value = "human")]
        format: OutputFormat,
    },
    /// Lint database schemas for anti-patterns and issues
    Lint {
        /// Path to dbhelper config file (toml)
        #[arg(default_value = "dbhelper.toml")]
        config: PathBuf,

        /// Only lint a specific database by name
        #[arg(long)]
        database: Option<String>,

        /// Output format
        #[arg(long, default_value = "human")]
        format: OutputFormat,
    },
    /// Suggest schema optimizations
    Optimize {
        /// Path to dbhelper config file (toml)
        #[arg(default_value = "dbhelper.toml")]
        config: PathBuf,

        /// Only analyze a specific database by name
        #[arg(long)]
        database: Option<String>,

        /// Output format
        #[arg(long, default_value = "human")]
        format: OutputFormat,
    },
    /// Validate a config file
    Check {
        /// Path to dbhelper config file (toml)
        #[arg(default_value = "dbhelper.toml")]
        config: PathBuf,
    },
    /// Initialize a new dbhelper.toml config file
    Init,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = run(cli).await;
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(2);
        }
    }
}

async fn run(cli: Cli) -> Result<i32, CliError> {
    match cli.command {
        Commands::Diff {
            config,
            database,
            format,
        } => cmd_diff(&config, database.as_deref(), &format),
        Commands::Lint {
            config,
            database,
            format,
        } => cmd_lint(&config, database.as_deref(), &format),
        Commands::Optimize {
            config,
            database,
            format,
        } => cmd_optimize(&config, database.as_deref(), &format),
        Commands::Check { config } => cmd_check(&config),
        Commands::Init => cmd_init(),
    }
}

/// Load config and resolve all paths relative to the config file.
fn load_config(path: &Path) -> Result<Config, CliError> {
    let mut cfg = Config::load(path)?;
    cfg.resolve_paths(path);
    Ok(cfg)
}

/// Parse migrations for all sources of a database, grouped by target schema.
/// Returns a map of schema_name -> Vec<(source_label, Schema)>.
fn parse_database_schemas(
    db: &DatabaseConfig,
    cfg: &Config,
) -> Result<HashMap<String, Vec<(String, Schema)>>, CliError> {
    let by_schema = Config::sources_by_schema(db);
    let mut result: HashMap<String, Vec<(String, Schema)>> = HashMap::new();

    for (schema_name, sources) in &by_schema {
        if cfg.is_schema_ignored(schema_name) {
            continue;
        }
        for source in sources {
            let label = format!("{} ({})", source.orm, source.migrations.display());
            let mut schema = parser::parse_migrations(&source.migrations, db.engine)?;
            schema.name = schema_name.clone();

            // Filter out ignored tables
            schema.tables.retain(|t| !cfg.is_table_ignored(&t.name));

            result
                .entry(schema_name.clone())
                .or_default()
                .push((label, schema));
        }
    }

    Ok(result)
}

/// Merge multiple schemas targeting the same namespace into one combined schema.
fn merge_schemas(schemas: &[(String, Schema)]) -> Schema {
    if schemas.len() == 1 {
        return schemas[0].1.clone();
    }

    let name = schemas[0].1.name.clone();
    let mut merged = Schema::new(&name);

    for (_label, schema) in schemas {
        for e in &schema.enums {
            if !merged.enums.iter().any(|existing| existing.name == e.name) {
                merged.enums.push(e.clone());
            }
        }
        for t in &schema.tables {
            if !merged.tables.iter().any(|existing| existing.name == t.name) {
                merged.tables.push(t.clone());
            }
        }
    }

    merged
}

/// Whether to use color output.
fn use_color() -> bool {
    std::io::stdout().is_terminal()
}

fn colored(text: &str, code: &str) -> String {
    if use_color() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn red(text: &str) -> String {
    colored(text, "31")
}

fn yellow(text: &str) -> String {
    colored(text, "33")
}

fn green(text: &str) -> String {
    colored(text, "32")
}

fn cyan(text: &str) -> String {
    colored(text, "36")
}

// ── diff command ─────────────────────────────────────────────────────────────

fn cmd_diff(
    config_path: &Path,
    database: Option<&str>,
    format: &OutputFormat,
) -> Result<i32, CliError> {
    let cfg = load_config(config_path)?;
    let databases = cfg.filter_databases(database);

    if databases.is_empty() {
        eprintln!("No matching database found");
        return Ok(2);
    }

    let mut has_changes = false;
    let mut all_output: Vec<serde_json::Value> = Vec::new();

    for db in &databases {
        let schemas_by_ns = parse_database_schemas(db, &cfg)?;

        for (schema_name, source_schemas) in &schemas_by_ns {
            // Detect cross-source conflicts
            let conflict_input: Vec<(&str, &Schema)> = source_schemas
                .iter()
                .map(|(label, schema)| (label.as_str(), schema))
                .collect();
            let conflicts = diff::detect_conflicts(&conflict_input);

            // Merge all sources into a single compiled schema
            let compiled = merge_schemas(source_schemas);

            // Load previous state if it exists
            let state_path = cfg
                .output_dir
                .join(&db.name)
                .join(format!("{schema_name}.json"));

            let previous = if state_path.exists() {
                let contents = std::fs::read_to_string(&state_path).map_err(|e| {
                    dbhelper_core::error::DiffError::ReadState {
                        path: state_path.clone(),
                        source: e,
                    }
                })?;
                serde_json::from_str::<Schema>(&contents).map_err(|e| {
                    dbhelper_core::error::DiffError::ParseState {
                        path: state_path.clone(),
                        source: e,
                    }
                })?
            } else {
                Schema::new(schema_name)
            };

            let schema_diff = diff::diff(&previous, &compiled);

            if !schema_diff.is_empty() || !conflicts.is_empty() {
                has_changes = true;
            }

            match format {
                OutputFormat::Human => {
                    print_diff_human(db, schema_name, &schema_diff, &conflicts);
                }
                OutputFormat::Json => {
                    all_output.push(serde_json::json!({
                        "database": db.name,
                        "schema": schema_name,
                        "changes": schema_diff.changes,
                        "conflicts": conflicts,
                    }));
                }
            }

            // Save the new state
            if let Some(parent) = state_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(&compiled)?;
            std::fs::write(&state_path, json).map_err(|e| {
                dbhelper_core::error::DiffError::WriteState {
                    path: state_path,
                    source: e,
                }
            })?;
        }
    }

    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&all_output)?);
    }

    Ok(if has_changes { 1 } else { 0 })
}

fn print_diff_human(
    db: &DatabaseConfig,
    schema_name: &str,
    diff: &SchemaDiff,
    conflicts: &[String],
) {
    println!(
        "{} {} schema '{}'",
        cyan("database"),
        cyan(&db.name),
        schema_name
    );

    if diff.is_empty() && conflicts.is_empty() {
        println!("  {}", green("no changes"));
        return;
    }

    for conflict in conflicts {
        println!("  {} {}", red("CONFLICT:"), conflict);
    }

    for change in &diff.changes {
        println!("  {}", format_change(change));
    }
    println!();
}

fn format_change(change: &Change) -> String {
    match change {
        Change::AddTable(name) => format!("{} table {}", green("+"), green(name)),
        Change::DropTable(name) => format!("{} table {}", red("-"), red(name)),
        Change::AddColumn { table, column } => {
            format!("{} column {}.{}", green("+"), table, green(column))
        }
        Change::DropColumn { table, column } => {
            format!("{} column {}.{}", red("-"), table, red(column))
        }
        Change::AlterColumn {
            table,
            column,
            description,
        } => format!(
            "{} column {}.{}: {}",
            yellow("~"),
            table,
            column,
            description
        ),
        Change::AddIndex { table, index } => {
            format!("{} index {} on {}", green("+"), green(index), table)
        }
        Change::DropIndex { table, index } => {
            format!("{} index {} on {}", red("-"), red(index), table)
        }
        Change::AddForeignKey { table, name } => {
            format!("{} foreign key {} on {}", green("+"), green(name), table)
        }
        Change::DropForeignKey { table, name } => {
            format!("{} foreign key {} on {}", red("-"), red(name), table)
        }
        Change::AddCheckConstraint { table, name } => {
            format!(
                "{} check constraint {} on {}",
                green("+"),
                green(name),
                table
            )
        }
        Change::DropCheckConstraint { table, name } => {
            format!("{} check constraint {} on {}", red("-"), red(name), table)
        }
        Change::AddUniqueConstraint { table, name } => {
            format!(
                "{} unique constraint {} on {}",
                green("+"),
                green(name),
                table
            )
        }
        Change::DropUniqueConstraint { table, name } => {
            format!("{} unique constraint {} on {}", red("-"), red(name), table)
        }
        Change::AddEnum { name } => format!("{} enum {}", green("+"), green(name)),
        Change::DropEnum { name } => format!("{} enum {}", red("-"), red(name)),
        Change::AlterEnum { name, description } => {
            format!("{} enum {}: {}", yellow("~"), name, description)
        }
        Change::ChangePrimaryKey { table, .. } => {
            format!("{} primary key on {}", yellow("~"), table)
        }
    }
}

// ── lint command ─────────────────────────────────────────────────────────────

fn cmd_lint(
    config_path: &Path,
    database: Option<&str>,
    format: &OutputFormat,
) -> Result<i32, CliError> {
    let cfg = load_config(config_path)?;
    let databases = cfg.filter_databases(database);

    if databases.is_empty() {
        eprintln!("No matching database found");
        return Ok(2);
    }

    let mut total_warnings = 0;
    let mut has_errors = false;
    let mut all_output: Vec<serde_json::Value> = Vec::new();

    for db in &databases {
        let schemas_by_ns = parse_database_schemas(db, &cfg)?;

        for (schema_name, source_schemas) in &schemas_by_ns {
            let compiled = merge_schemas(source_schemas);
            let warnings = lint::lint(&compiled, &cfg.lint);

            if !warnings.is_empty() {
                total_warnings += warnings.len();
                if warnings.iter().any(|w| w.severity == Severity::Error) {
                    has_errors = true;
                }
            }

            match format {
                OutputFormat::Human => {
                    print_lint_human(db, schema_name, &warnings);
                }
                OutputFormat::Json => {
                    all_output.push(serde_json::json!({
                        "database": db.name,
                        "schema": schema_name,
                        "warnings": warnings,
                    }));
                }
            }
        }
    }

    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&all_output)?);
    } else if total_warnings == 0 {
        println!("{}", green("No lint warnings found."));
    } else {
        println!(
            "\n{} lint warning(s) found.",
            if has_errors {
                red(&total_warnings.to_string())
            } else {
                yellow(&total_warnings.to_string())
            }
        );
    }

    Ok(if has_errors {
        2
    } else if total_warnings > 0 {
        1
    } else {
        0
    })
}

fn print_lint_human(db: &DatabaseConfig, schema_name: &str, warnings: &[LintWarning]) {
    if warnings.is_empty() {
        return;
    }

    println!(
        "{} {} schema '{}'",
        cyan("database"),
        cyan(&db.name),
        schema_name
    );

    for w in warnings {
        let severity_str = match w.severity {
            Severity::Error => red(&format!("[{}]", w.severity)),
            Severity::Warning => yellow(&format!("[{}]", w.severity)),
            Severity::Info => format!("[{}]", w.severity),
        };

        let location = match (&w.table, &w.column) {
            (Some(t), Some(c)) => format!(" {t}.{c}:"),
            (Some(t), None) => format!(" {t}:"),
            _ => String::new(),
        };

        println!(
            "  {} {}{} {} ({})",
            severity_str, w.rule, location, w.message, w.rule
        );
    }
}

// ── optimize command ─────────────────────────────────────────────────────────

fn cmd_optimize(
    config_path: &Path,
    database: Option<&str>,
    format: &OutputFormat,
) -> Result<i32, CliError> {
    let cfg = load_config(config_path)?;
    let databases = cfg.filter_databases(database);

    if databases.is_empty() {
        eprintln!("No matching database found");
        return Ok(2);
    }

    let mut total_suggestions = 0;
    let mut all_output: Vec<serde_json::Value> = Vec::new();

    for db in &databases {
        let schemas_by_ns = parse_database_schemas(db, &cfg)?;

        for (schema_name, source_schemas) in &schemas_by_ns {
            let compiled = merge_schemas(source_schemas);
            let suggestions = optimize::analyze(&compiled);
            total_suggestions += suggestions.len();

            match format {
                OutputFormat::Human => {
                    print_optimize_human(db, schema_name, &suggestions);
                }
                OutputFormat::Json => {
                    all_output.push(serde_json::json!({
                        "database": db.name,
                        "schema": schema_name,
                        "suggestions": suggestions,
                    }));
                }
            }
        }
    }

    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&all_output)?);
    } else if total_suggestions == 0 {
        println!("{}", green("No optimization suggestions."));
    } else {
        println!(
            "\n{} optimization suggestion(s).",
            yellow(&total_suggestions.to_string())
        );
    }

    Ok(if total_suggestions > 0 { 1 } else { 0 })
}

fn print_optimize_human(db: &DatabaseConfig, schema_name: &str, suggestions: &[Suggestion]) {
    if suggestions.is_empty() {
        return;
    }

    println!(
        "{} {} schema '{}'",
        cyan("database"),
        cyan(&db.name),
        schema_name
    );

    for s in suggestions {
        let location = match (&s.table, &s.column) {
            (Some(t), Some(c)) => format!(" {t}.{c}:"),
            (Some(t), None) => format!(" {t}:"),
            _ => String::new(),
        };
        println!("  {} [{}]{} {}", yellow("*"), s.rule, location, s.message);
    }
}

// ── check command ────────────────────────────────────────────────────────────

fn cmd_check(config_path: &Path) -> Result<i32, CliError> {
    let cfg = load_config(config_path)?;
    println!(
        "{} {} database(s) defined",
        green("Config OK:"),
        cfg.databases.len()
    );
    for db in &cfg.databases {
        let by_schema = Config::sources_by_schema(db);
        println!(
            "  {} ({}): {} source(s) across {} schema(s)",
            db.name,
            db.engine,
            db.sources.len(),
            by_schema.len(),
        );
        for (schema, sources) in &by_schema {
            if sources.len() > 1 {
                println!(
                    "    {} schema '{}': {} sources — will check for conflicts",
                    yellow("⚠"),
                    schema,
                    sources.len()
                );
            }
        }
    }
    Ok(0)
}

// ── init command ─────────────────────────────────────────────────────────────

fn cmd_init() -> Result<i32, CliError> {
    let target = Path::new("dbhelper.toml");
    if target.exists() {
        eprintln!("dbhelper.toml already exists in this directory");
        return Ok(1);
    }

    let template = r#"# dbhelper configuration
#
# This file describes the database landscape for your project.
# Run `dbhelper diff` to compare newly generated state against the last known state.

# Directory where dbhelper stores compiled schema state and snapshots.
output_dir = ".dbhelper/state"

[[databases]]
name = "myapp"
engine = "postgres"
# connection_url = "postgresql://user:pass@localhost:5432/myapp"

[[databases.sources]]
orm = "drizzle"
migrations = "drizzle/migrations"
# schema = "public"  # defaults to "public" for Postgres

# Add more sources targeting the same or different schemas:
# [[databases.sources]]
# orm = "alembic"
# migrations = "alembic/versions"
# schema = "analytics"

# Lint configuration (optional)
# [lint]
# disabled_rules = ["unbounded-text"]
# naming_convention = "snake_case"
# enum_value_threshold = 20
# require_timestamps = false

# Ignore patterns (optional)
# [ignore]
# tables = ["schema_migrations", "__drizzle_migrations"]
# schemas = ["pg_catalog", "information_schema"]
"#;

    std::fs::write(target, template)?;
    println!("{} created dbhelper.toml", green("✓"));
    Ok(0)
}
