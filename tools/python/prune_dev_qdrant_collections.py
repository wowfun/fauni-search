#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import shutil
from pathlib import Path
from typing import Any

from local_status import ROOT, discover_pids, env


PLAYWRIGHT_STAGE_PREFIX = "vector_space_stage_playwright-"


def repo_relative(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path.resolve())


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


def collection_timestamp(path: Path) -> tuple[int, int, str]:
    matches = [int(value) for value in re.findall(r"(?:^|[-_])(\d{13,})(?:[-_]|$)", path.name)]
    timestamp_ms = matches[0] if matches else int(path.stat().st_mtime_ns / 1_000_000)
    return timestamp_ms, path.stat().st_mtime_ns, path.name


def collection_paths(qdrant_storage_dir: Path) -> list[Path]:
    collections_dir = qdrant_storage_dir / "collections"
    if not collections_dir.is_dir():
        return []
    return sorted(path for path in collections_dir.iterdir() if path.is_dir())


def alias_target(value: Any) -> str | None:
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        for key in ("collection_name", "collection", "target", "target_collection"):
            target = value.get(key)
            if isinstance(target, str):
                return target
    return None


def prune_aliases(qdrant_storage_dir: Path, deleted_collection_names: set[str]) -> list[str]:
    aliases_path = qdrant_storage_dir / "aliases" / "data.json"
    if not aliases_path.is_file() or not deleted_collection_names:
        return []

    try:
        payload = json.loads(aliases_path.read_text())
    except (OSError, json.JSONDecodeError):
        return []

    if not isinstance(payload, dict):
        return []

    kept: dict[str, Any] = {}
    deleted_aliases: list[str] = []
    for alias, target_payload in payload.items():
        target = alias_target(target_payload)
        if target in deleted_collection_names:
            deleted_aliases.append(alias)
        else:
            kept[alias] = target_payload

    if deleted_aliases:
        aliases_path.write_text(
            f"{json.dumps(kept, ensure_ascii=False, sort_keys=True)}\n",
            encoding="utf8",
        )

    return sorted(deleted_aliases)


def build_payload(max_count: int, keep_count: int) -> dict[str, Any]:
    qdrant_storage_dir = Path(env("QDRANT_STORAGE_DIR")).resolve()
    collections = collection_paths(qdrant_storage_dir)
    playwright_stage = [
        path for path in collections if path.name.startswith(PLAYWRIGHT_STAGE_PREFIX)
    ]

    newest_first = sorted(playwright_stage, key=collection_timestamp, reverse=True)
    to_delete = newest_first[keep_count:] if len(collections) > max_count else []
    services = running_services() if to_delete else {}
    blocked = bool(services)

    deleted_collections: list[str] = []
    deleted_aliases: list[str] = []
    if to_delete and not blocked:
        deleted_names = {path.name for path in to_delete}
        deleted_aliases = prune_aliases(qdrant_storage_dir, deleted_names)
        for path in to_delete:
            if path.exists():
                shutil.rmtree(path)
                deleted_collections.append(path.name)

    status = "blocked" if blocked else "pruned" if deleted_collections else "skipped"
    return {
        "status": status,
        "config_source": env("FAUNI_CONFIG_SOURCE"),
        "qdrant_storage_dir": repo_relative(qdrant_storage_dir),
        "max_count": max_count,
        "keep_count": keep_count,
        "collection_count": len(collections),
        "playwright_stage_count": len(playwright_stage),
        "planned_delete_count": len(to_delete),
        "deleted_collection_count": len(deleted_collections),
        "deleted_collections": deleted_collections,
        "deleted_alias_count": len(deleted_aliases),
        "deleted_aliases": deleted_aliases,
        "blocked": blocked,
        "running_services": services,
    }


def print_text(payload: dict[str, Any]) -> None:
    print(f"Config: {payload['config_source']}")
    print(f"Qdrant: {payload['qdrant_storage_dir']}")
    print(
        "[info] Collections: "
        f"{payload['collection_count']} total, "
        f"{payload['playwright_stage_count']} playwright stage"
    )

    if payload["blocked"]:
        print("[error] Refusing to prune Qdrant collections while services are running")
        for service, pids in dict(payload["running_services"]).items():
            print(f"[info] {service}: {','.join(str(pid) for pid in pids)}")
        return

    if payload["status"] == "skipped":
        print(
            "[ok] Prune skipped; collection count is within "
            f"the configured max ({payload['max_count']})"
        )
        return

    print(
        "[ok] Pruned "
        f"{payload['deleted_collection_count']} collection(s) and "
        f"{payload['deleted_alias_count']} alias(es); kept newest "
        f"{payload['keep_count']} playwright stage collection(s)"
    )


def positive_int(raw: str) -> int:
    value = int(raw)
    if value <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--max-count", type=positive_int, default=500)
    parser.add_argument("--keep-count", type=positive_int, default=100)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    if args.keep_count >= args.max_count:
        raise SystemExit("--keep-count must be less than --max-count")

    payload = build_payload(args.max_count, args.keep_count)
    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print_text(payload)
    return 1 if payload["blocked"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
