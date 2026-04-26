mod client;
mod error;
mod serve;
mod status;
mod web;

use clap::{Parser, Subcommand};
use error::{invalid_input, CliFailure};
use serve::{run_serve, ServeArgs};
use status::run_status;
use web::run_web;

#[derive(Parser, Debug)]
#[command(name = "faus", about = "FauniSearch product CLI")]
struct Cli {
    #[arg(long, global = true)]
    base_url: Option<String>,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    debug: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Serve(ServeArgs),
    Status,
    Web,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing();

    if let Err(error) = run(cli).await {
        error.write();
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), CliFailure> {
    match cli.command {
        Commands::Serve(args) => {
            if cli.base_url.is_some() {
                return Err(CliFailure::human(invalid_input(
                    "`--base-url` is for client commands; use `faus serve --host` and `--port`.",
                )));
            }
            if cli.json {
                return Err(CliFailure::human(invalid_input(
                    "`faus serve --json` is not defined for the foreground server command yet.",
                )));
            }
            run_serve(args, cli.debug).await.map_err(CliFailure::human)
        }
        Commands::Status => run_status(cli.base_url, cli.json, cli.debug).await,
        Commands::Web => run_web(cli.base_url, cli.json, cli.debug).await,
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
