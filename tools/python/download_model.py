#!/usr/bin/env python3
from __future__ import annotations

import argparse

from runtime_config import load_merged_runtime_config, resolve_local_sidecar_active_model


def main() -> int:
    parser = argparse.ArgumentParser(description="Download a Hugging Face model snapshot")
    parser.add_argument("--hf-repo-id")
    parser.add_argument("--version")
    args = parser.parse_args()

    hf_repo_id = args.hf_repo_id
    version = args.version
    if not hf_repo_id or not version:
        config = load_merged_runtime_config()
        default_model_id, default_version = resolve_local_sidecar_active_model(config)
        hf_repo_id = hf_repo_id or default_model_id
        version = version or default_version

    from huggingface_hub import snapshot_download

    snapshot_path = snapshot_download(repo_id=hf_repo_id, revision=version)
    print(f"[ok] Downloaded snapshot: {snapshot_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
