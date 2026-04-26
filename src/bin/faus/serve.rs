use crate::error::invalid_input;
use clap::Args;
use fauni_search::{
    build_app, new_state, resolve_local_sidecar_active_model_from_env, spawn_runtime_maintenance,
};
use reqwest::Client;
use std::{
    env,
    error::Error,
    fs::{self, File},
    future::Future,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    process::{Child, Command, Stdio},
    time::Duration,
};

const DEFAULT_APP_HOST: &str = "127.0.0.1";
const DEFAULT_APP_PORT: u16 = 53210;
const HTTP_READY_ATTEMPTS: usize = 30;

pub(crate) type CliResult<T> = Result<T, Box<dyn Error>>;
pub(crate) type ReadyHook =
    Box<dyn FnOnce(ServeReady) -> Pin<Box<dyn Future<Output = CliResult<()>>>>>;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ServeOutput {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub(crate) struct ServeReady {
    pub(crate) base_url: String,
    pub(crate) health_url: String,
}

#[derive(Args, Debug)]
pub(crate) struct ServeArgs {
    #[arg(long, value_name = "HOST", help = "Rust App API listen host")]
    pub(crate) host: Option<String>,
    #[arg(long, value_name = "PORT", help = "Rust App API listen port")]
    pub(crate) port: Option<u16>,
    #[arg(long, help = "Use the isolated .env.dev runtime profile")]
    pub(crate) dev: bool,
}

impl ServeArgs {
    pub(crate) fn default_runtime() -> Self {
        Self {
            host: None,
            port: None,
            dev: false,
        }
    }
}

pub(crate) async fn run_serve(args: ServeArgs, debug: bool) -> CliResult<()> {
    run_serve_with_ready_hook(args, debug, ServeOutput::Stdout, None).await
}

pub(crate) async fn run_serve_with_ready_hook(
    args: ServeArgs,
    debug: bool,
    output: ServeOutput,
    ready_hook: Option<ReadyHook>,
) -> CliResult<()> {
    let repo_root = find_repo_root(&env::current_dir()?)?;
    env::set_current_dir(&repo_root)?;
    prepend_local_bin_to_path(&repo_root)?;

    let loaded_env = load_selected_env(&repo_root, args.dev)?;
    apply_serve_overrides(&args);
    ensure_default_app_bind_env();
    ensure_runtime_generation_ready(&repo_root)?;
    ensure_local_sidecar_model_env()?;

    let app_host = required_env("APP_HOST")?;
    let app_port = required_env("APP_PORT")?;
    let app_bind = format!("{app_host}:{app_port}");
    let app_base_url = format!("http://{app_bind}");

    let dev_log_dir = resolve_env_path(&repo_root, &required_env("DEV_LOG_DIR")?);
    fs::create_dir_all(&dev_log_dir).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to create log directory {}: {error}",
                dev_log_dir.display()
            ),
        )
    })?;

    if debug {
        serve_log(
            output,
            format!("[debug] Config: {}", loaded_env.source.display()),
        );
        serve_log(output, format!("[debug] Mode:   {}", loaded_env.mode));
        serve_log(output, format!("[debug] Logs:   {}", dev_log_dir.display()));
    }

    let client = Client::new();
    let mut children = ManagedChildren::default();

    ensure_qdrant_ready(
        &repo_root,
        &dev_log_dir,
        &client,
        &mut children,
        debug,
        output,
    )
    .await?;
    ensure_sidecar_ready(
        &repo_root,
        &dev_log_dir,
        &client,
        &mut children,
        debug,
        output,
    )
    .await?;

    let listener = tokio::net::TcpListener::bind(&app_bind)
        .await
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("Failed to bind Rust server at {app_bind}: {error}"),
            )
        })?;
    let state = new_state().await?;
    spawn_runtime_maintenance(state.clone());
    let app = build_app(state);

    serve_log(output, "[ok] FauniSearch runtime is ready");
    serve_log(
        output,
        format!("[info] Config:  {}", loaded_env.source.display()),
    );
    serve_log(output, format!("[info] App:     {app_base_url}/health"));
    serve_log(
        output,
        format!("[info] OpenAPI: {app_base_url}/openapi.json"),
    );
    serve_log(output, format!("[info] Sidecar: {}", sidecar_health_url()?));
    serve_log(
        output,
        format!("[info] Qdrant:  {}", qdrant_collections_url()?),
    );
    serve_log(output, format!("[info] Logs:    {}", dev_log_dir.display()));

    let ready = ServeReady {
        health_url: format!("{app_base_url}/health"),
        base_url: app_base_url,
    };

    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
    });

    if let Some(ready_hook) = ready_hook {
        if let Err(error) = ready_hook(ready).await {
            server_task.abort();
            return Err(error);
        }
    }

    server_task.await.map_err(|error| {
        io::Error::new(io::ErrorKind::Other, format!("server task failed: {error}"))
    })??;
    children.shutdown();
    Ok(())
}

async fn ensure_qdrant_ready(
    repo_root: &Path,
    dev_log_dir: &Path,
    client: &Client,
    children: &mut ManagedChildren,
    debug: bool,
    output: ServeOutput,
) -> CliResult<()> {
    let collections_url = qdrant_collections_url()?;
    if http_ok(client, &collections_url).await {
        serve_log(
            output,
            format!("[info] Reusing existing Qdrant at {collections_url}"),
        );
        return Ok(());
    }

    let qdrant_host = required_env("QDRANT_HOST")?;
    let qdrant_port = required_env("QDRANT_PORT")?;
    let qdrant_storage_dir = resolve_env_path(repo_root, &required_env("QDRANT_STORAGE_DIR")?);
    fs::create_dir_all(&qdrant_storage_dir).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to create Qdrant storage directory {}: {error}",
                qdrant_storage_dir.display()
            ),
        )
    })?;

    let log_path = dev_log_dir.join("qdrant.log");
    let mut command = Command::new("qdrant");
    command
        .env("QDRANT__SERVICE__HOST", &qdrant_host)
        .env("QDRANT__SERVICE__HTTP_PORT", &qdrant_port)
        .env("QDRANT__STORAGE__STORAGE_PATH", &qdrant_storage_dir);

    if debug {
        serve_log(
            output,
            format!(
                "[debug] Starting Qdrant at {qdrant_host}:{qdrant_port}; storage {}",
                qdrant_storage_dir.display()
            ),
        );
    } else {
        serve_log(
            output,
            format!("[info] Starting Qdrant at {qdrant_host}:{qdrant_port}"),
        );
    }

    let child = spawn_logged_child("Qdrant", command, &log_path)?;
    children.push("Qdrant", child);
    wait_http_ok(client, "Qdrant", &collections_url, HTTP_READY_ATTEMPTS).await?;
    serve_log(output, format!("[ok] Qdrant is ready at {collections_url}"));
    Ok(())
}

async fn ensure_sidecar_ready(
    repo_root: &Path,
    dev_log_dir: &Path,
    client: &Client,
    children: &mut ManagedChildren,
    debug: bool,
    output: ServeOutput,
) -> CliResult<()> {
    let health_url = sidecar_health_url()?;
    if http_ok(client, &health_url).await {
        serve_log(
            output,
            format!("[info] Reusing existing Python sidecar at {health_url}"),
        );
        return Ok(());
    }

    let python = repo_root.join(".venv/bin/python");
    if !python.exists() {
        return Err(invalid_input(format!(
            ".venv is missing; run scripts/local/bootstrap-linux.sh{} first",
            env_arg_hint()
        ))
        .into());
    }

    let sidecar_src = repo_root.join("sidecar/src");
    let log_path = dev_log_dir.join("sidecar.log");
    let mut command = Command::new(&python);
    command
        .arg("-m")
        .arg("fauni_sidecar")
        .env("PYTHONPATH", &sidecar_src);

    if debug {
        serve_log(
            output,
            format!(
                "[debug] Starting Python sidecar with {}; PYTHONPATH={}",
                python.display(),
                sidecar_src.display()
            ),
        );
    } else {
        serve_log(
            output,
            format!("[info] Starting Python sidecar at {health_url}"),
        );
    }

    let child = spawn_logged_child("Python sidecar", command, &log_path)?;
    children.push("Python sidecar", child);
    wait_http_ok(client, "Python sidecar", &health_url, HTTP_READY_ATTEMPTS).await?;
    serve_log(
        output,
        format!("[ok] Python sidecar is ready at {health_url}"),
    );
    Ok(())
}

fn serve_log(output: ServeOutput, message: impl std::fmt::Display) {
    match output {
        ServeOutput::Stdout => println!("{message}"),
        ServeOutput::Stderr => eprintln!("{message}"),
    }
}

async fn http_ok(client: &Client, url: &str) -> bool {
    match client.get(url).timeout(Duration::from_secs(1)).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

async fn wait_http_ok(client: &Client, label: &str, url: &str, attempts: usize) -> CliResult<()> {
    for _ in 0..attempts {
        if http_ok(client, url).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("{label} did not become ready at {url}"),
    )
    .into())
}

fn spawn_logged_child(
    label: &'static str,
    mut command: Command,
    log_path: &Path,
) -> CliResult<Child> {
    let stdout = File::create(log_path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to create {label} log {}: {error}",
                log_path.display()
            ),
        )
    })?;
    let stderr = stdout.try_clone()?;
    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    command.spawn().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Failed to start {label}; see {}: {error}",
                log_path.display()
            ),
        )
        .into()
    })
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut terminate =
            signal(SignalKind::terminate()).expect("SIGTERM handler should install");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[derive(Default)]
struct ManagedChildren {
    children: Vec<ManagedChild>,
}

impl ManagedChildren {
    fn push(&mut self, label: &'static str, child: Child) {
        self.children.push(ManagedChild { label, child });
    }

    fn shutdown(&mut self) {
        while let Some(mut child) = self.children.pop() {
            child.kill_and_wait();
        }
    }
}

impl Drop for ManagedChildren {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct ManagedChild {
    label: &'static str,
    child: Child,
}

impl ManagedChild {
    fn kill_and_wait(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => {
                eprintln!("[warn] Failed to inspect {} process: {error}", self.label);
                return;
            }
        }
        if let Err(error) = self.child.kill() {
            eprintln!("[warn] Failed to stop {} process: {error}", self.label);
        }
        let _ = self.child.wait();
    }
}

fn load_selected_env(repo_root: &Path, dev: bool) -> CliResult<LoadedEnv> {
    let selected = select_env_file(repo_root, dev)?;
    let contents = fs::read_to_string(&selected.source).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to read env file {}: {error}",
                selected.source.display()
            ),
        )
    })?;
    for (key, value) in parse_env_assignments(&contents)? {
        env::set_var(key, value);
    }
    env::set_var("FAUNI_CONFIG_SOURCE", &selected.source);
    env::set_var("FAUNI_CONFIG_TARGET", &selected.source);
    env::set_var("FAUNI_CONFIG_MODE", selected.mode);
    Ok(selected)
}

pub(crate) fn load_default_env(repo_root: &Path) -> CliResult<()> {
    let _ = load_selected_env(repo_root, false)?;
    Ok(())
}

#[derive(Debug)]
struct LoadedEnv {
    source: PathBuf,
    mode: &'static str,
}

fn select_env_file(repo_root: &Path, dev: bool) -> CliResult<LoadedEnv> {
    let (source, mode, missing_message) = if dev {
        (
            repo_root.join(".env.dev"),
            "dev",
            ".env.dev is missing; run scripts/local/bootstrap-linux.sh --dev first".to_string(),
        )
    } else if let Some(env_file) = env::var_os("FAUNI_ENV_FILE").filter(|value| !value.is_empty()) {
        (
            resolve_path(repo_root, Path::new(&env_file)),
            "custom",
            format!(
                "FAUNI_ENV_FILE does not exist: {}",
                Path::new(&env_file).display()
            ),
        )
    } else {
        (
            repo_root.join(".env"),
            "default",
            ".env is missing; run scripts/local/bootstrap-linux.sh first".to_string(),
        )
    };

    if !source.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, missing_message).into());
    }

    Ok(LoadedEnv { source, mode })
}

fn parse_env_assignments(contents: &str) -> Result<Vec<(String, String)>, io::Error> {
    let mut entries = Vec::new();
    for (index, raw_line) in contents.lines().enumerate() {
        let mut line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim();
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid env assignment on line {}", index + 1),
            ));
        };
        let key = raw_key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            || key.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid env key `{key}` on line {}", index + 1),
            ));
        }
        entries.push((key.to_string(), unquote_env_value(raw_value.trim())));
    }
    Ok(entries)
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn apply_serve_overrides(args: &ServeArgs) {
    if let Some(host) = &args.host {
        env::set_var("APP_HOST", host);
    }
    if let Some(port) = args.port {
        env::set_var("APP_PORT", port.to_string());
    }
}

fn ensure_default_app_bind_env() {
    if env::var_os("APP_HOST").is_none() {
        env::set_var("APP_HOST", DEFAULT_APP_HOST);
    }
    if env::var_os("APP_PORT").is_none() {
        env::set_var("APP_PORT", DEFAULT_APP_PORT.to_string());
    }
}

fn ensure_runtime_generation_ready(repo_root: &Path) -> CliResult<()> {
    let app_runtime_dir = resolve_env_path(repo_root, &required_env("APP_RUNTIME_DIR")?);
    let qdrant_storage_dir = resolve_env_path(repo_root, &required_env("QDRANT_STORAGE_DIR")?);
    let runtime_config_path = app_runtime_dir.join("runtime-config.json");

    if runtime_config_path.exists() {
        fs::create_dir_all(&qdrant_storage_dir)?;
        return Ok(());
    }

    if dir_has_entries(&app_runtime_dir) || dir_has_entries(&qdrant_storage_dir) {
        return Err(invalid_input(format!(
            "Legacy runtime data detected; run scripts/local/cutover-runtime.sh{} before starting services",
            env_arg_hint()
        ))
        .into());
    }

    fs::create_dir_all(&app_runtime_dir)?;
    fs::create_dir_all(&qdrant_storage_dir)?;
    fs::write(&runtime_config_path, "{}\n").map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to initialize runtime config {}: {error}",
                runtime_config_path.display()
            ),
        )
    })?;
    Ok(())
}

fn ensure_local_sidecar_model_env() -> CliResult<()> {
    let (model_id, model_revision) = resolve_local_sidecar_active_model_from_env()?;
    env::set_var("EMBEDDING_MODEL_ID", &model_id);
    env::set_var("EMBEDDING_MODEL_REVISION", &model_revision);
    Ok(())
}

fn dir_has_entries(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

fn qdrant_collections_url() -> Result<String, io::Error> {
    let qdrant_url = match env::var("QDRANT_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            let host = required_env("QDRANT_HOST")?;
            let port = required_env("QDRANT_PORT")?;
            let value = format!("http://{host}:{port}");
            env::set_var("QDRANT_URL", &value);
            value
        }
    };
    Ok(format!("{}/collections", qdrant_url.trim_end_matches('/')))
}

fn sidecar_health_url() -> Result<String, io::Error> {
    Ok(format!(
        "http://{}:{}/health",
        required_env("SIDECAR_HOST")?,
        required_env("SIDECAR_PORT")?
    ))
}

pub(crate) fn required_env(name: &'static str) -> Result<String, io::Error> {
    env::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Missing required environment variable {name}; source .env or use faus serve --dev"
            ),
        )
    })
}

pub(crate) fn find_repo_root(start: &Path) -> Result<PathBuf, io::Error> {
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").exists() && candidate.join("fauni.config.json").exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "Could not find FauniSearch repo root from {}; run faus from the project tree",
            start.display()
        ),
    ))
}

fn prepend_local_bin_to_path(repo_root: &Path) -> Result<(), io::Error> {
    let local_bin = repo_root.join("tools/local/bin");
    if !local_bin.is_dir() {
        return Ok(());
    }

    let mut paths = vec![local_bin];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    let joined = env::join_paths(paths).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Failed to update PATH for local tools: {error}"),
        )
    })?;
    env::set_var("PATH", joined);
    Ok(())
}

fn resolve_env_path(repo_root: &Path, value: &str) -> PathBuf {
    resolve_path(repo_root, Path::new(value))
}

fn resolve_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn env_arg_hint() -> &'static str {
    match env::var("FAUNI_CONFIG_MODE").ok().as_deref() {
        Some("dev") => " --dev",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn parses_basic_env_file() {
        let parsed = parse_env_assignments(
            r#"
            # comment
            APP_HOST=127.0.0.1
            export APP_PORT=53210
            QUOTED="hello world"
            SINGLE='value'
            "#,
        )
        .expect("env file should parse");

        assert_eq!(
            parsed,
            vec![
                ("APP_HOST".to_string(), "127.0.0.1".to_string()),
                ("APP_PORT".to_string(), "53210".to_string()),
                ("QUOTED".to_string(), "hello world".to_string()),
                ("SINGLE".to_string(), "value".to_string()),
            ]
        );
    }

    #[test]
    fn rejects_invalid_env_key() {
        let error = parse_env_assignments("1APP=bad").expect_err("invalid key should fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn serve_overrides_host_and_port_without_touching_base_url() {
        let _lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let restore = EnvRestore::capture(&["APP_HOST", "APP_PORT", "FAUS_BASE_URL"]);

        env::set_var("APP_HOST", "127.0.0.1");
        env::set_var("APP_PORT", "53210");
        env::set_var("FAUS_BASE_URL", "http://127.0.0.1:39010");
        apply_serve_overrides(&ServeArgs {
            host: Some("0.0.0.0".to_string()),
            port: Some(39099),
            dev: false,
        });

        assert_eq!(env::var("APP_HOST").unwrap(), "0.0.0.0");
        assert_eq!(env::var("APP_PORT").unwrap(), "39099");
        assert_eq!(env::var("FAUS_BASE_URL").unwrap(), "http://127.0.0.1:39010");

        restore.restore();
    }

    #[test]
    fn qdrant_url_can_be_derived_from_host_port() {
        let _lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let restore = EnvRestore::capture(&["QDRANT_URL", "QDRANT_HOST", "QDRANT_PORT"]);

        env::remove_var("QDRANT_URL");
        env::set_var("QDRANT_HOST", "127.0.0.1");
        env::set_var("QDRANT_PORT", "56333");

        assert_eq!(
            qdrant_collections_url().unwrap(),
            "http://127.0.0.1:56333/collections"
        );

        restore.restore();
    }

    struct EnvRestore {
        values: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                values: keys
                    .iter()
                    .map(|key| (*key, env::var_os(key)))
                    .collect::<Vec<_>>(),
            }
        }

        fn restore(self) {
            for (key, value) in self.values {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}
