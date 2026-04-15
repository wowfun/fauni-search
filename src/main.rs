use fauni_search::{build_app, new_state, spawn_runtime_maintenance};
use std::{env, error::Error, io};
use tracing::info;

fn required_env(name: &'static str) -> Result<String, io::Error> {
    env::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Missing required environment variable {name}; source .env or use scripts/local/run.sh"),
        )
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let host = required_env("APP_HOST")?;
    let port = required_env("APP_PORT")?;
    let bind = format!("{host}:{port}");
    let state = new_state().await?;
    spawn_runtime_maintenance(state.clone());
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    info!("FauniSearch app listening on http://{bind}");

    axum::serve(listener, app).await?;
    Ok(())
}
