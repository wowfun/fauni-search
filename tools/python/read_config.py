#!/usr/bin/env python3
from __future__ import annotations

import argparse

from runtime_config import load_merged_runtime_config, resolve_local_sidecar_active_model


def main() -> int:
    parser = argparse.ArgumentParser(description="Read values from merged Fauni config")
    subparsers = parser.add_subparsers(dest="command", required=True)

    local_sidecar_parser = subparsers.add_parser(
        "local-sidecar-active", help="Print the active local_sidecar model field"
    )
    local_sidecar_parser.add_argument("--field", choices=["model_id", "version"], required=True)

    args = parser.parse_args()
    config = load_merged_runtime_config()

    if args.command == "local-sidecar-active":
        model_id, version = resolve_local_sidecar_active_model(config)
        print(model_id if args.field == "model_id" else version)
        return 0

    raise ValueError(f"Unsupported command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
