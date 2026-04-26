#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import urllib.request
from pathlib import Path
from urllib.error import URLError


ROOT = Path.cwd()


def http_ok(url: str, timeout: float = 1.0) -> bool:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            return 200 <= response.status < 500
    except URLError:
        return False


def pid_is_alive(pid: int) -> bool:
    return (Path("/proc") / str(pid)).is_dir()


def pid_cmd(pid: int) -> str:
    try:
        return (Path("/proc") / str(pid) / "cmdline").read_bytes().replace(b"\0", b" ").decode()
    except OSError:
        return ""


def pid_cwd(pid: int) -> Path | None:
    try:
        return Path(os.readlink(Path("/proc") / str(pid) / "cwd")).resolve()
    except OSError:
        return None


def pid_belongs_to_repo(pid: int) -> bool:
    cwd = pid_cwd(pid)
    if cwd is None:
        return False
    try:
        cwd.relative_to(ROOT)
        return True
    except ValueError:
        return cwd == ROOT


def pids_for_port(port: str) -> list[int]:
    try:
        output = subprocess.check_output(
            ["lsof", "-nP", f"-tiTCP:{port}", "-sTCP:LISTEN"],
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return []
    return [int(line) for line in output.splitlines() if line.strip().isdigit()]


def cmd_matches_service(service: str, cmd: str, ui_port: str) -> bool:
    if service == "app":
        return (
            "target/debug/fauni-search" in cmd
            or "target/release/fauni-search" in cmd
            or "target/debug/faus serve" in cmd
            or "target/release/faus serve" in cmd
            or "cargo run" in cmd
        )
    if service == "sidecar":
        return "-m fauni_sidecar" in cmd
    if service == "ui":
        return "vite" in cmd and f"--port {ui_port}" in cmd
    if service == "qdrant":
        return cmd.startswith("qdrant ") or "/qdrant" in cmd
    return False


def read_pid_file(path: Path) -> int | None:
    try:
        raw = path.read_text().strip()
    except OSError:
        return None
    if not raw.isdigit():
        return None
    pid = int(raw)
    return pid if pid_is_alive(pid) else None


def discover_pids(service: str, port: str, pid_file: Path, ui_port: str) -> list[int]:
    pids: list[int] = []

    pid = read_pid_file(pid_file)
    if pid is not None and pid_belongs_to_repo(pid) and cmd_matches_service(service, pid_cmd(pid), ui_port):
        pids.append(pid)

    for pid in pids_for_port(port):
        cmd = pid_cmd(pid)
        if pid_belongs_to_repo(pid) and cmd_matches_service(service, cmd, ui_port):
            pids.append(pid)

    deduped = []
    for pid in pids:
        if pid not in deduped:
            deduped.append(pid)
    return deduped


def env(name: str) -> str:
    value = os.environ.get(name)
    if value is None:
        raise SystemExit(f"Missing required environment variable: {name}")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    args = parser.parse_args()

    app_host = env("APP_HOST")
    app_port = env("APP_PORT")
    sidecar_host = env("SIDECAR_HOST")
    sidecar_port = env("SIDECAR_PORT")
    ui_host = env("UI_HOST")
    ui_port = env("UI_PORT")
    qdrant_host = env("QDRANT_HOST")
    qdrant_port = env("QDRANT_PORT")
    qdrant_url = env("QDRANT_URL").rstrip("/")
    log_dir = Path(env("DEV_LOG_DIR"))

    service_defs = {
        "app": {
            "url": f"http://{app_host}:{app_port}/health",
            "port": int(app_port),
            "pid_file": log_dir / "app.pid",
            "log": log_dir / "app.log",
        },
        "sidecar": {
            "url": f"http://{sidecar_host}:{sidecar_port}/health",
            "port": int(sidecar_port),
            "pid_file": log_dir / "sidecar.pid",
            "log": log_dir / "sidecar.log",
        },
        "ui": {
            "url": f"http://{ui_host}:{ui_port}/",
            "port": int(ui_port),
            "pid_file": log_dir / "ui.pid",
            "log": log_dir / "ui.log",
        },
        "qdrant": {
            "url": f"{qdrant_url}/collections",
            "port": int(qdrant_port),
            "pid_file": log_dir / "qdrant.pid",
            "log": log_dir / "qdrant.log",
        },
    }

    services = {}
    for name, config in service_defs.items():
        pids = discover_pids(name, str(config["port"]), config["pid_file"], ui_port)
        services[name] = {
            "url": config["url"],
            "ready": http_ok(config["url"]),
            "pid": pids[0] if pids else None,
            "pids": pids,
            "pid_file": str(config["pid_file"]),
            "log": str(config["log"]),
        }

    payload = {
        "config_source": os.environ.get("FAUNI_CONFIG_SOURCE", ""),
        "config_mode": os.environ.get("FAUNI_CONFIG_MODE", ""),
        "services": services,
    }

    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True))
        return 0

    print(f"Config: {payload['config_source']}")
    for name, service in services.items():
        state = "ready" if service["ready"] else "not_ready"
        pids = ",".join(str(pid) for pid in service["pids"]) or "-"
        print(f"{name}: {state} {service['url']} pid={pids} log={service['log']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
