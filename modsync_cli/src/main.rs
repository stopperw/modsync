use clap::{Parser, Subcommand};
use sync::SyncCommand;

mod sync;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Sync(SyncCommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    pretty_env_logger::init();

    let args = Args::parse();

    match args.commands {
        Commands::Sync(mut sync) => sync.run().await,
    }
}
