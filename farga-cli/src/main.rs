mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "farga", about = "Farga context substrate CLI")]
struct Cli {
    #[arg(long, default_value = "http://localhost:7500")]
    server: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Context {
        #[command(subcommand)]
        kind: commands::context::ContextKind,
    },
    Signals {
        #[arg(long)]
        project: String,
        #[arg(long, default_value = "24")]
        since_hours: u64,
    },
    Artifacts {
        #[arg(long)]
        project: String,
    },
    Proposals {
        #[command(subcommand)]
        action: commands::proposals::ProposalAction,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let base = cli.server.clone();
    match cli.command {
        Commands::Context { kind } => commands::context::run(&base, kind).await,
        Commands::Signals { project, since_hours } => commands::signals::run(&base, &project, since_hours).await,
        Commands::Artifacts { project } => commands::artifacts::run(&base, &project).await,
        Commands::Proposals { action } => commands::proposals::run(&base, action).await,
    }
}
