from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any


def _repo_config_path() -> Path:
    path = os.environ.get("FAUNI_CONFIG_PATH")
    if path and path.strip():
        return Path(path)
    return Path.cwd() / "fauni.config.json"


def _runtime_config_path() -> Path:
    runtime_dir = os.environ.get("APP_RUNTIME_DIR")
    if not runtime_dir or not runtime_dir.strip():
        raise ValueError(
            "Missing required environment variable APP_RUNTIME_DIR; source .env or use scripts/local/run.sh"
        )
    return Path(runtime_dir) / "runtime-config.json"


def _load_json(path: Path, *, required: bool) -> dict[str, Any]:
    if not path.exists():
        if required:
            raise ValueError(f"Fauni config file was not found: {path}")
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"Fauni config file must decode to an object: {path}")
    return data


def _deep_merge(base: dict[str, Any], overlay: dict[str, Any]) -> dict[str, Any]:
    merged = dict(base)
    for key, value in overlay.items():
        if isinstance(value, dict) and isinstance(merged.get(key), dict):
            merged[key] = _deep_merge(merged[key], value)
        else:
            merged[key] = value
    return merged


def load_merged_runtime_config() -> dict[str, Any]:
    repo = _load_json(_repo_config_path(), required=True)
    runtime = _load_json(_runtime_config_path(), required=False)
    return _deep_merge(repo, runtime)


def resolve_local_sidecar_active_model(config: dict[str, Any]) -> tuple[str, str]:
    providers = config.get("provider")
    if not isinstance(providers, dict):
        raise ValueError("Fauni config must define provider.local_sidecar.")
    provider = providers.get("local_sidecar")
    if not isinstance(provider, dict):
        raise ValueError("Fauni config must define provider.local_sidecar.")
    active_model = str(provider.get("active_model", "")).strip()
    if not active_model:
        raise ValueError("provider.local_sidecar.active_model must be a non-empty string.")
    models = provider.get("models")
    if not isinstance(models, dict):
        raise ValueError("provider.local_sidecar.models must be an object.")
    model = models.get(active_model)
    if not isinstance(model, dict):
        raise ValueError(
            f"provider.local_sidecar.active_model points to missing model {active_model}."
        )
    if model.get("enabled", True) is False:
        raise ValueError(f"provider.local_sidecar.models.{active_model} is disabled.")
    version = str(model.get("version", "main")).strip() or "main"
    return active_model, version
