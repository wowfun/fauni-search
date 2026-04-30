mod client;
mod error;
mod find;
mod import;
mod jobs;
mod library;
mod search;
mod serve;
mod sources;
mod status;
mod web;

use clap::{Parser, Subcommand};
use error::{invalid_input, CliFailure};
use find::{run_find, FindArgs};
use import::{run_import, ImportArgs};
use jobs::{run_jobs, JobsArgs};
use library::{run_library, LibraryArgs};
use search::{run_search, SearchArgs};
use serve::{run_serve, ServeArgs};
use sources::{run_sources, SourcesArgs};
use status::run_status;
use web::run_web;

#[derive(Parser, Debug)]
#[command(
    name = "faus",
    about = "FauniSearch product CLI",
    long_about = "FauniSearch product CLI for starting the local runtime and using the App API.",
    after_help = "Examples:\n  faus serve\n  faus status\n  faus library list\n  faus sources roots list --library-id demo\n  faus import --library-id demo report.pdf\n  faus search --library-id demo --text \"terminal screen\"\n  faus find ./notes --text \"quarterly revenue\"\n  faus jobs list\n  faus web"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "URL",
        help = "Use a FauniSearch App API base URL for client commands"
    )]
    base_url: Option<String>,
    #[arg(long, global = true, help = "Print stable machine-readable JSON")]
    json: bool,
    #[arg(long, global = true, help = "Include CLI-side diagnostic metadata")]
    debug: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Start the headless local runtime")]
    Serve(ServeArgs),
    #[command(
        about = "Inspect an existing App API runtime",
        long_about = "Inspect an existing FauniSearch App API runtime. This command does not start local processes.",
        after_help = "Examples:\n  faus status\n  faus --base-url http://127.0.0.1:54210 status\n  faus --json status"
    )]
    Status,
    #[command(about = "Manage libraries through the App API")]
    Library(LibraryArgs),
    #[command(about = "Submit local paths for import through the App API")]
    Import(ImportArgs),
    #[command(about = "Search with text or a local query file through the App API")]
    Search(SearchArgs),
    #[command(about = "Find Asset results inside a local folder through the App API")]
    Find(FindArgs),
    #[command(about = "Manage library source roots and source inventory through the App API")]
    Sources(SourcesArgs),
    #[command(about = "Manage runtime jobs through the App API")]
    Jobs(JobsArgs),
    #[command(
        about = "Open the Web experience for a local or existing runtime",
        long_about = "Open the FauniSearch Web experience. With an explicit base URL it connects to that App API; without one it may start the default local runtime. It uses built Web assets and does not start Vite.",
        after_help = "Examples:\n  faus web\n  faus --base-url http://127.0.0.1:54210 web\n  faus --json web"
    )]
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
        Commands::Library(args) => run_library(args, cli.base_url, cli.json, cli.debug).await,
        Commands::Import(args) => run_import(args, cli.base_url, cli.json, cli.debug).await,
        Commands::Search(args) => run_search(args, cli.base_url, cli.json, cli.debug).await,
        Commands::Find(args) => run_find(args, cli.base_url, cli.json, cli.debug).await,
        Commands::Sources(args) => run_sources(args, cli.base_url, cli.json, cli.debug).await,
        Commands::Jobs(args) => run_jobs(args, cli.base_url, cli.json, cli.debug).await,
        Commands::Web => run_web(cli.base_url, cli.json, cli.debug).await,
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
