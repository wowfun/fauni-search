use std::{
    fs,
    path::{Path, PathBuf},
};

const AUDIT_ROOTS: &[&str] = &["specs", "docs", "src", "ui/src", "ui/tests", "tests"];
const AUDIT_EXTENSIONS: &[&str] = &["md", "rs", "ts", "tsx"];
const BANNED_TERMS: &[&str] = &[
    "model-defaults",
    "model-overrides",
    "resolved-models",
    "multivector",
    "repr_kind",
];

#[test]
fn public_terminology_audit_rejects_legacy_terms() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for root in AUDIT_ROOTS {
        collect_audit_files(&repo_root.join(root), &mut files);
    }

    let mut findings = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(repo_root)
            .expect("audit path should remain inside the repo");
        if should_skip_file(relative) {
            continue;
        }

        let content = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", relative.display()));
        for (index, line) in content.lines().enumerate() {
            for term in BANNED_TERMS {
                if line.contains(term) && !is_allowed_exception(relative, line, term) {
                    findings.push(format!("{}:{}: {}", relative.display(), index + 1, term));
                }
            }
        }
    }

    assert!(
        findings.is_empty(),
        "legacy terminology remains in public-facing files:\n{}",
        findings.join("\n")
    );
}

fn collect_audit_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if !dir.exists() {
        return;
    }

    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("directory entry should be readable").path();
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_audit_files(&path, files);
            continue;
        }

        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| AUDIT_EXTENSIONS.contains(&ext))
        {
            files.push(path);
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    path.ends_with(".references") || path.ends_with("ui/dist")
}

fn should_skip_file(path: &Path) -> bool {
    path.ends_with("tests/terminology_audit.rs")
}

fn is_allowed_exception(path: &Path, line: &str, term: &str) -> bool {
    path.ends_with("src/qdrant.rs") && term == "multivector" && line.contains("multivector_config")
}
