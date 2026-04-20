#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

from local_status import ROOT, discover_pids, env


def repo_relative(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path.resolve())


def active_alias_targets(qdrant_storage_dir: Path) -> set[str]:
    aliases_path = qdrant_storage_dir / "aliases" / "data.json"
    if not aliases_path.is_file():
        return set()

    try:
        payload = json.loads(aliases_path.read_text())
    except (OSError, json.JSONDecodeError):
        return set()

    if not isinstance(payload, dict):
        return set()

    return {
        str(target)
        for target in payload.values()
        if isinstance(target, str) and target.strip()
    }


def legacy_archives(runtime_root: Path) -> list[Path]:
    if not runtime_root.is_dir():
        return []
    return sorted(
        path
        for path in runtime_root.iterdir()
        if path.is_dir() and path.name.startswith("legacy-")
    )


def classify_legacy_collections(qdrant_storage_dir: Path) -> tuple[list[Path], list[str]]:
    collections_dir = qdrant_storage_dir / "collections"
    if not collections_dir.is_dir():
        return [], sorted(active_alias_targets(qdrant_storage_dir))

    active_targets = active_alias_targets(qdrant_storage_dir)
    legacy: list[Path] = []
    for path in sorted(collections_dir.iterdir()):
        if not path.is_dir():
            continue
        name = path.name
        if name in active_targets:
            continue
        if name.startswith("vector_space_stage_"):
            continue
        if (
            name.startswith("index_")
            or name.startswith("text_search_")
            or name.startswith("vector_space_")
        ):
            legacy.append(path)

    return legacy, sorted(active_targets)


def running_services() -> dict[str, list[int]]:
    log_dir = Path(env("DEV_LOG_DIR"))
    ui_port = env("UI_PORT")
    services = {
        "app": (env("APP_PORT"), log_dir / "app.pid"),
        "sidecar": (env("SIDECAR_PORT"), log_dir / "sidecar.pid"),
        "ui": (ui_port, log_dir / "ui.pid"),
        "qdrant": (env("QDRANT_PORT"), log_dir / "qdrant.pid"),
    }

    active: dict[str, list[int]] = {}
    for service, (port, pid_file) in services.items():
        pids = discover_pids(service, port, pid_file, ui_port)
        if pids:
            active[service] = pids
    return active


def delete_paths(paths: list[Path]) -> list[str]:
    deleted: list[str] = []
    for path in paths:
        if not path.exists():
            continue
        shutil.rmtree(path)
        deleted.append(repo_relative(path))
    return deleted


def build_payload(execute: bool, blocked: bool) -> dict[str, object]:
    app_runtime_dir = Path(env("APP_RUNTIME_DIR")).resolve()
    qdrant_storage_dir = Path(env("QDRANT_STORAGE_DIR")).resolve()
    runtime_root = app_runtime_dir.parent
    archive_paths = legacy_archives(runtime_root)
    collection_paths, active_targets = classify_legacy_collections(qdrant_storage_dir)
    services = running_services() if execute else {}

    payload: dict[str, object] = {
        "config_source": env("FAUNI_CONFIG_SOURCE"),
        "runtime_root": repo_relative(runtime_root),
        "app_runtime_dir": repo_relative(app_runtime_dir),
        "qdrant_storage_dir": repo_relative(qdrant_storage_dir),
        "legacy_archives": [repo_relative(path) for path in archive_paths],
        "legacy_collections": [path.name for path in collection_paths],
        "active_alias_targets": active_targets,
        "mode": "execute" if execute else "scan",
        "blocked": blocked,
        "running_services": services,
        "deleted_archives": [],
        "deleted_collections": [],
    }

    if execute and not blocked:
        payload["deleted_archives"] = delete_paths(archive_paths)
        payload["deleted_collections"] = delete_paths(collection_paths)

    archive_count = len(payload["deleted_archives"] if execute and not blocked else payload["legacy_archives"])
    collection_count = len(payload["deleted_collections"] if execute and not blocked else payload["legacy_collections"])
    payload["status"] = (
        "blocked"
        if blocked
        else "cleaned"
        if execute
        else "scanned"
    )
    payload["summary"] = {
        "legacy_archive_count": archive_count,
        "legacy_collection_count": collection_count,
    }
    return payload


def print_text(payload: dict[str, object]) -> None:
    print(f"Config: {payload['config_source']}")
    print(f"Runtime root: {payload['runtime_root']}")

    if payload["blocked"]:
        print("[error] Refusing to delete legacy runtime data while services are running")
        for service, pids in dict(payload["running_services"]).items():
            pid_list = ",".join(str(pid) for pid in pids)
            print(f"[info] {service}: {pid_list}")
        return

    action = "Deleted" if payload["mode"] == "execute" else "Found"
    archives = list(payload["deleted_archives"] if payload["mode"] == "execute" else payload["legacy_archives"])
    collections = list(payload["deleted_collections"] if payload["mode"] == "execute" else payload["legacy_collections"])

    print(f"[info] {action} {len(archives)} legacy archive(s)")
    for archive in archives:
        print(f"  - {archive}")

    print(f"[info] {action} {len(collections)} legacy collection(s)")
    for collection in collections:
        print(f"  - {collection}")

    if payload["active_alias_targets"]:
        print("[info] Active alias targets kept:")
        for target in list(payload["active_alias_targets"]):
            print(f"  - {target}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    parser.add_argument("--execute", action="store_true", help="delete discovered legacy data")
    args = parser.parse_args()

    blocked = False
    if args.execute:
        blocked = bool(running_services())

    payload = build_payload(execute=args.execute, blocked=blocked)
    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print_text(payload)

    return 1 if blocked else 0


if __name__ == "__main__":
    raise SystemExit(main())
