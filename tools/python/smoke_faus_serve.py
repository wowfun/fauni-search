#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import queue
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from pathlib import Path
from urllib.error import URLError


ROOT = Path.cwd()
READY_TIMEOUT_SECONDS = 90.0
PORT_RELEASE_TIMEOUT_SECONDS = 20.0
PORT_PROBE_TIMEOUT_SECONDS = 0.5


def env(name: str) -> str:
    value = os.environ.get(name)
    if value is None or value == "":
        raise SystemExit(f"[error] Missing required environment variable {name}")
    return value


def endpoint_config() -> dict[str, object]:
    app_host = env("APP_HOST")
    app_port = int(env("APP_PORT"))
    sidecar_host = env("SIDECAR_HOST")
    sidecar_port = int(env("SIDECAR_PORT"))
    qdrant_url = env("QDRANT_URL").rstrip("/")
    ui_host = env("UI_HOST")
    ui_port = int(env("UI_PORT"))
    qdrant_port = int(env("QDRANT_PORT"))
    qdrant_host = env("QDRANT_HOST")

    return {
        "app_url": f"http://{app_host}:{app_port}",
        "sidecar_url": f"http://{sidecar_host}:{sidecar_port}",
        "qdrant_url": qdrant_url,
        "ports": {
            "app": (app_host, app_port),
            "sidecar": (sidecar_host, sidecar_port),
            "qdrant": (qdrant_host, qdrant_port),
            "vite_ui": (ui_host, ui_port),
        },
    }


def emit(message: str, *, json_mode: bool) -> None:
    print(message, file=sys.stderr if json_mode else sys.stdout, flush=True)


def port_open(host: str, port: int) -> bool:
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(PORT_PROBE_TIMEOUT_SECONDS)
    try:
        return sock.connect_ex((host, port)) == 0
    finally:
        sock.close()


def require_ports_free(ports: dict[str, tuple[str, int]]) -> None:
    occupied = [
        f"{name}({host}:{port})"
        for name, (host, port) in ports.items()
        if port_open(host, port)
    ]
    if occupied:
        raise SystemExit(
            "[error] .env.dev runtime ports are already occupied: "
            + ", ".join(occupied)
            + "; stop the dev runtime before running smoke-faus-serve"
        )


def get_json(url: str, timeout: float = 5.0) -> tuple[int, dict]:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except URLError as exc:
        raise RuntimeError(f"GET {url} failed: {exc}") from exc


def start_output_reader(process: subprocess.Popen[str]) -> tuple[queue.Queue[str], list[str]]:
    output_queue: queue.Queue[str] = queue.Queue()
    lines: list[str] = []

    def read_output() -> None:
        assert process.stdout is not None
        for line in process.stdout:
            line = line.rstrip("\n")
            lines.append(line)
            output_queue.put(line)

    thread = threading.Thread(target=read_output, daemon=True)
    thread.start()
    return output_queue, lines


def drain_output(output_queue: queue.Queue[str], *, json_mode: bool) -> None:
    while True:
        try:
            line = output_queue.get_nowait()
        except queue.Empty:
            return
        emit(line, json_mode=json_mode)


def wait_for_ready(
    process: subprocess.Popen[str],
    output_queue: queue.Queue[str],
    config: dict[str, object],
    *,
    json_mode: bool,
) -> dict[str, dict]:
    app_url = str(config["app_url"])
    sidecar_url = str(config["sidecar_url"])
    qdrant_url = str(config["qdrant_url"])
    deadline = time.monotonic() + READY_TIMEOUT_SECONDS
    probes = {
        "app_health": (f"{app_url}/health", None),
        "openapi": (f"{app_url}/openapi.json", None),
        "runtime_status": (f"{app_url}/runtime/status", None),
        "sidecar_health": (f"{sidecar_url}/health", None),
        "qdrant_collections": (f"{qdrant_url}/collections", None),
    }

    while time.monotonic() < deadline:
        drain_output(output_queue, json_mode=json_mode)
        exit_code = process.poll()
        if exit_code is not None:
            raise RuntimeError(f"faus serve exited before readiness with code {exit_code}")

        all_ready = True
        for name, (url, payload) in list(probes.items()):
            if payload is not None:
                continue
            try:
                status, data = get_json(url)
            except RuntimeError:
                all_ready = False
                continue
            if status < 200 or status >= 300:
                all_ready = False
                continue
            probes[name] = (url, data)
        if all_ready:
            return {name: payload for name, (_url, payload) in probes.items() if payload is not None}
        time.sleep(0.5)

    raise RuntimeError("faus serve did not become ready in time")


def validate_payloads(payloads: dict[str, dict]) -> None:
    openapi = payloads["openapi"]
    if openapi.get("openapi") != "3.1.0" or "paths" not in openapi:
        raise RuntimeError("openapi.json did not expose the expected OpenAPI 3.1 paths payload")

    runtime_status = payloads["runtime_status"].get("data") or {}
    missing = {"app", "qdrant", "providers"} - set(runtime_status)
    if missing:
        raise RuntimeError(f"runtime/status missing keys: {sorted(missing)}")


def terminate_process(process: subprocess.Popen[str]) -> int:
    if process.poll() is None:
        process.send_signal(signal.SIGTERM)
    try:
        return process.wait(timeout=PORT_RELEASE_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        return process.wait(timeout=5.0)


def wait_for_ports_released(ports: dict[str, tuple[str, int]]) -> dict[str, bool]:
    deadline = time.monotonic() + PORT_RELEASE_TIMEOUT_SECONDS
    release_state = {name: False for name in ports}
    while time.monotonic() < deadline:
        release_state = {
            name: not port_open(host, port) for name, (host, port) in ports.items()
        }
        if all(release_state.values()):
            return release_state
        time.sleep(0.5)
    return release_state


def output_contains_required_lines(lines: list[str]) -> dict[str, bool]:
    required = {
        "app": "[info] App:",
        "web": "[info] Web:",
        "openapi": "[info] OpenAPI:",
        "sidecar": "[info] Sidecar:",
        "qdrant": "[info] Qdrant:",
        "logs": "[info] Logs:",
    }
    return {
        name: any(marker in line for line in lines)
        for name, marker in required.items()
    }


def run_smoke(*, json_mode: bool) -> dict[str, object]:
    if os.environ.get("FAUNI_CONFIG_MODE") != "dev":
        raise SystemExit("[error] smoke_faus_serve.py must run with .env.dev selected")

    config = endpoint_config()
    ports = config["ports"]  # type: ignore[assignment]
    assert isinstance(ports, dict)
    require_ports_free(ports)

    command = [str(ROOT / "target/debug/faus"), "serve", "--dev"]
    emit("[info] Starting target/debug/faus serve --dev", json_mode=json_mode)
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=os.environ.copy(),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    output_queue, output_lines = start_output_reader(process)

    payloads: dict[str, dict] = {}
    exit_code: int | None = None
    try:
        payloads = wait_for_ready(process, output_queue, config, json_mode=json_mode)
        validate_payloads(payloads)
        if port_open(*ports["vite_ui"]):
            raise RuntimeError("Vite UI port is open; faus serve must not start Vite")
    finally:
        exit_code = terminate_process(process)
        drain_output(output_queue, json_mode=json_mode)

    released = wait_for_ports_released(ports)
    if not all(released.values()):
        raise RuntimeError(f"faus serve left .env.dev ports open: {released}")

    required_output = output_contains_required_lines(output_lines)
    if not all(required_output.values()):
        raise RuntimeError(f"faus serve output missed expected route lines: {required_output}")

    runtime_status = payloads["runtime_status"].get("data") or {}
    sidecar_health = payloads["sidecar_health"]
    return {
        "status": "ok",
        "command": "target/debug/faus serve --dev",
        "config": os.environ.get("FAUNI_CONFIG_SOURCE"),
        "app_url": config["app_url"],
        "sidecar_url": config["sidecar_url"],
        "qdrant_url": config["qdrant_url"],
        "http": {
            "health": payloads["app_health"].get("status"),
            "openapi": payloads["openapi"].get("openapi"),
            "runtime_status_keys": sorted(runtime_status.keys()),
            "sidecar_status": sidecar_health.get("status"),
            "qdrant_status": payloads["qdrant_collections"].get("status"),
        },
        "vite_ui_started": False,
        "process_exit_code": exit_code,
        "ports_released": released,
        "output_contains": required_output,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print compact machine-readable JSON")
    args = parser.parse_args()

    try:
        summary = run_smoke(json_mode=args.json)
    except Exception as exc:
        if args.json:
            print(
                json.dumps(
                    {
                        "status": "error",
                        "error": {
                            "code": "faus_serve_smoke_failed",
                            "message": str(exc),
                        },
                    },
                    ensure_ascii=False,
                    sort_keys=True,
                )
            )
        else:
            print(f"[error] {exc}", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
