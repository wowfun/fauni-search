use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn unique_temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    std::env::temp_dir().join(format!("fauni-local-cleanup-{name}-{millis}"))
}

fn write_env_file(root: &Path, runtime_root: &Path) -> PathBuf {
    let env_path = root.join("cleanup.env");
    let contents = format!(
        "APP_HOST=127.0.0.1\n\
APP_PORT=39010\n\
SIDECAR_HOST=127.0.0.1\n\
SIDECAR_PORT=39011\n\
UI_HOST=127.0.0.1\n\
UI_PORT=39012\n\
QDRANT_HOST=127.0.0.1\n\
QDRANT_PORT=39013\n\
QDRANT_URL=http://127.0.0.1:39013\n\
DEV_LOG_DIR={logs}\n\
APP_RUNTIME_DIR={app_runtime}\n\
QDRANT_STORAGE_DIR={qdrant}\n",
        logs = root.join("logs").display(),
        app_runtime = runtime_root.join("app").display(),
        qdrant = runtime_root.join("qdrant").display(),
    );
    fs::write(&env_path, contents).unwrap();
    env_path
}

fn run_cleanup_script(env_file: &Path, args: &[&str]) -> Value {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/local/cleanup-legacy-runtime.sh");
    let output = Command::new("bash")
        .arg(script)
        .args(args)
        .env("FAUNI_ENV_FILE", env_file)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cleanup script should run");

    assert!(
        output.status.success(),
        "cleanup script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cleanup script should return json")
}

#[test]
fn cleanup_legacy_runtime_scan_and_execute_only_remove_legacy_artifacts() {
    let root = unique_temp_dir("scan-execute");
    let runtime_root = root.join("runtime");
    fs::create_dir_all(&root).unwrap();
    let env_file = write_env_file(&root, &runtime_root);
    let qdrant_storage = runtime_root.join("qdrant");

    fs::create_dir_all(root.join("logs")).unwrap();
    fs::create_dir_all(runtime_root.join("app")).unwrap();
    fs::create_dir_all(qdrant_storage.join("aliases")).unwrap();
    fs::create_dir_all(qdrant_storage.join("collections")).unwrap();

    let legacy_archive = runtime_root.join("legacy-20260420-000000");
    fs::create_dir_all(legacy_archive.join("app")).unwrap();
    fs::create_dir_all(legacy_archive.join("qdrant")).unwrap();

    let index_collection = qdrant_storage.join("collections/index_demo-lib_1");
    let text_search_collection = qdrant_storage.join("collections/text_search_demo-lib");
    let direct_vector_space_collection =
        qdrant_storage.join("collections/vector_space_demo-lib_legacy");
    let active_stage_collection =
        qdrant_storage.join("collections/vector_space_stage_demo-lib_current_job_000001");
    let orphan_stage_collection =
        qdrant_storage.join("collections/vector_space_stage_demo-lib_orphan_job_000002");
    fs::create_dir_all(&index_collection).unwrap();
    fs::create_dir_all(&text_search_collection).unwrap();
    fs::create_dir_all(&direct_vector_space_collection).unwrap();
    fs::create_dir_all(&active_stage_collection).unwrap();
    fs::create_dir_all(&orphan_stage_collection).unwrap();
    fs::write(
        qdrant_storage.join("aliases/data.json"),
        r#"{"vector_space_demo-lib_current":"vector_space_stage_demo-lib_current_job_000001"}"#,
    )
    .unwrap();

    let scan = run_cleanup_script(&env_file, &["--json"]);
    assert_eq!(scan["status"], "scanned");
    assert_eq!(scan["legacy_archives"].as_array().unwrap().len(), 1);
    assert_eq!(scan["legacy_collections"].as_array().unwrap().len(), 3);
    assert!(scan["legacy_collections"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "index_demo-lib_1"));
    assert!(scan["active_alias_targets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "vector_space_stage_demo-lib_current_job_000001"));
    assert!(legacy_archive.exists());
    assert!(index_collection.exists());
    assert!(text_search_collection.exists());
    assert!(direct_vector_space_collection.exists());
    assert!(active_stage_collection.exists());
    assert!(orphan_stage_collection.exists());

    let execute = run_cleanup_script(&env_file, &["--json", "--execute"]);
    assert_eq!(execute["status"], "cleaned");
    assert_eq!(execute["deleted_archives"].as_array().unwrap().len(), 1);
    assert_eq!(execute["deleted_collections"].as_array().unwrap().len(), 3);
    assert!(!legacy_archive.exists());
    assert!(!index_collection.exists());
    assert!(!text_search_collection.exists());
    assert!(!direct_vector_space_collection.exists());
    assert!(active_stage_collection.exists());
    assert!(orphan_stage_collection.exists());

    let _ = fs::remove_dir_all(root);
}
