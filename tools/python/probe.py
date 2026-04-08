#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib
import json
import socket
import sys
import urllib.request


def http_ok(args: argparse.Namespace) -> int:
    try:
        with urllib.request.urlopen(args.url, timeout=args.timeout) as response:
            return 0 if response.status < 500 else 1
    except Exception:
        return 1


def probe_port(host: str, port: int, timeout: float) -> str:
    sock = socket.socket()
    sock.settimeout(timeout)
    try:
        return "occupied" if sock.connect_ex((host, port)) == 0 else "free"
    except PermissionError:
        return "permission_denied"
    except OSError:
        return "error"
    finally:
        sock.close()


def port_status(args: argparse.Namespace) -> int:
    print(probe_port(args.host, args.port, args.timeout))
    return 0


def port_free(args: argparse.Namespace) -> int:
    return 0 if probe_port(args.host, args.port, args.timeout) == "free" else 1


def python_is(args: argparse.Namespace) -> int:
    major, minor = (int(part) for part in args.version.split(".", maxsplit=1))
    return 0 if sys.version_info[:2] == (major, minor) else 1


def import_modules(args: argparse.Namespace) -> int:
    try:
        for module_name in args.modules:
            importlib.import_module(module_name)
    except Exception as exc:
        print(f"{type(exc).__name__}: {exc}", file=sys.stderr)
        return 1
    return 0


def gpu_json(_args: argparse.Namespace) -> int:
    import torch

    result: dict[str, object] = {
        "torch": torch.__version__,
        "cuda": torch.version.cuda,
    }

    try:
        available = bool(torch.cuda.is_available())
        result["available"] = available
        result["count"] = torch.cuda.device_count() if available else 0
        if available:
            x = torch.randn((2, 3), device="cuda")
            y = x @ x.transpose(0, 1)
            result["smoke"] = float(y.sum().item())
    except Exception as exc:
        result["error"] = f"{type(exc).__name__}: {exc}"

    print(json.dumps(result))
    return 0


def parse_gpu_payload(payload: str) -> dict[str, object]:
    return json.loads(payload)


def gpu_json_available(args: argparse.Namespace) -> int:
    data = parse_gpu_payload(args.payload)
    return 0 if data.get("available") else 1


def gpu_json_summary(args: argparse.Namespace) -> int:
    data = parse_gpu_payload(args.payload)
    print(
        "torch {torch} / cuda {cuda} / devices {count} / smoke {smoke}".format(
            torch=data.get("torch"),
            cuda=data.get("cuda"),
            count=data.get("count"),
            smoke=data.get("smoke"),
        )
    )
    return 0


def gpu_json_details(args: argparse.Namespace) -> int:
    data = parse_gpu_payload(args.payload)
    print(
        data.get(
            "error",
            "torch {torch} / cuda {cuda} / devices {count}".format(
                torch=data.get("torch"),
                cuda=data.get("cuda"),
                count=data.get("count", 0),
            ),
        )
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Local FauniSearch probe helpers")
    subparsers = parser.add_subparsers(dest="command", required=True)

    http_parser = subparsers.add_parser("http-ok")
    http_parser.add_argument("url")
    http_parser.add_argument("--timeout", type=float, default=1.0)
    http_parser.set_defaults(func=http_ok)

    port_status_parser = subparsers.add_parser("port-status")
    port_status_parser.add_argument("host")
    port_status_parser.add_argument("port", type=int)
    port_status_parser.add_argument("--timeout", type=float, default=0.5)
    port_status_parser.set_defaults(func=port_status)

    port_free_parser = subparsers.add_parser("port-free")
    port_free_parser.add_argument("host")
    port_free_parser.add_argument("port", type=int)
    port_free_parser.add_argument("--timeout", type=float, default=0.5)
    port_free_parser.set_defaults(func=port_free)

    python_parser = subparsers.add_parser("python-is")
    python_parser.add_argument("version")
    python_parser.set_defaults(func=python_is)

    import_parser = subparsers.add_parser("import-modules")
    import_parser.add_argument("modules", nargs="+")
    import_parser.set_defaults(func=import_modules)

    gpu_parser = subparsers.add_parser("gpu-json")
    gpu_parser.set_defaults(func=gpu_json)

    gpu_available_parser = subparsers.add_parser("gpu-json-available")
    gpu_available_parser.add_argument("payload")
    gpu_available_parser.set_defaults(func=gpu_json_available)

    gpu_summary_parser = subparsers.add_parser("gpu-json-summary")
    gpu_summary_parser.add_argument("payload")
    gpu_summary_parser.set_defaults(func=gpu_json_summary)

    gpu_details_parser = subparsers.add_parser("gpu-json-details")
    gpu_details_parser.add_argument("payload")
    gpu_details_parser.set_defaults(func=gpu_json_details)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
