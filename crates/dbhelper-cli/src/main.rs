use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use dbhelper_core::config::Config;

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

    match cli.command {
        Commands::Diff {
            config,
            database,
            format: _,
        } => {
            let cfg = load_config(&config);
            println!(
                "Diffing {} database(s) from {}",
                match &database {
                    Some(name) => format!("'{name}'"),
                    None => format!("all {}", cfg.databases.len()),
                },
                config.display()
            );
            // TODO: for each database, parse migrations from all sources,
            // merge schemas per-schema, compare against output_dir state,
            // report diffs and cross-source conflicts
        }
        Commands::Lint {
            config,
            database,
            format: _,
        } => {
            let cfg = load_config(&config);
            println!(
                "Linting {} database(s) from {}",
                match &database {
                    Some(name) => format!("'{name}'"),
                    None => format!("all {}", cfg.databases.len()),
                },
                config.display()
            );
            // TODO: parse migrations, build schemas, run lint rules
        }
        Commands::Optimize {
            config,
            database,
            format: _,
        } => {
            let cfg = load_config(&config);
            println!(
                "Optimizing {} database(s) from {}",
                match &database {
                    Some(name) => format!("'{name}'"),
                    None => format!("all {}", cfg.databases.len()),
                },
                config.display()
            );
            // TODO: parse migrations, build schemas, run optimization analysis
        }
        Commands::Check { config } => {
            let cfg = load_config(&config);
            println!("Config OK: {} database(s) defined", cfg.databases.len());
            for db in &cfg.databases {
                let by_schema = Config::sources_by_schema(db);
                println!(
                    "  {} ({}): {} source(s) across {} schema(s)",
                    db.name,
                    format!("{:?}", db.engine).to_lowercase(),
                    db.sources.len(),
                    by_schema.len(),
                );
                for (schema, sources) in &by_schema {
                    if sources.len() > 1 {
                        println!(
                            "    ⚠ schema '{}': {} sources — will check for conflicts",
                            schema,
                            sources.len()
                        );
                    }
                }
            }
        }
        Commands::Init => {
            // TODO: generate a starter dbhelper.toml
            println!("TODO: generate dbhelper.toml");
        }
    }
}

fn load_config(path: &Path) -> Config {
    match Config::load(path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(2);
        }
    }
}
