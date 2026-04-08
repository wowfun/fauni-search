#!/usr/bin/env python3
from __future__ import annotations

import argparse

from huggingface_hub import snapshot_download


def main() -> int:
    parser = argparse.ArgumentParser(description="Download a Hugging Face model snapshot")
    parser.add_argument("repo_id")
    parser.add_argument("revision")
    args = parser.parse_args()

    snapshot_path = snapshot_download(repo_id=args.repo_id, revision=args.revision)
    print(f"[ok] Downloaded snapshot: {snapshot_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
