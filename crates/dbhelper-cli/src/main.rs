use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dbhelper", about = "Database linter, diff, and optimization tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Diff two database schemas
    Diff {
        /// Source connection URL or migration path
        from: String,
        /// Target connection URL or migration path
        to: String,
    },
    /// Lint a database schema
    Lint {
        /// Connection URL or migration path
        target: String,
    },
    /// Suggest schema optimizations
    Optimize {
        /// Connection URL or migration path
        target: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diff { from, to } => {
            println!("Diffing {from} -> {to}");
            // TODO: implement
        }
        Commands::Lint { target } => {
            println!("Linting {target}");
            // TODO: implement
        }
        Commands::Optimize { target } => {
            println!("Optimizing {target}");
            // TODO: implement
        }
    }
}
