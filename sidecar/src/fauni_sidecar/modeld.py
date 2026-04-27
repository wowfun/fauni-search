from __future__ import annotations

import io
import logging
from logging.handlers import RotatingFileHandler
import os
from pathlib import Path
import sys
import threading
from datetime import datetime, timezone
from typing import Any

import uvicorn
from fastapi import FastAPI
from pydantic import BaseModel, field_validator

from fauni_sidecar.app import SidecarApiError, create_app
from fauni_sidecar.runtime import (
    ColQwenRuntime,
    EmbeddingRuntime,
    LocalSidecarModelConfig,
    Qwen3VlEmbeddingRuntime,
    list_local_sidecar_model_configs_from_runtime_config,
    require_env,
    resolve_local_sidecar_model_config_from_runtime_config,
    utc_now,
)

MODELD_LOG_MAX_BYTES = 10 * 1024 * 1024
MODELD_LOG_BACKUP_COUNT = 5
MODELD_LOGGER_NAMES = (
    "",
    "uvicorn",
    "uvicorn.error",
    "uvicorn.access",
    "fauni_sidecar",
    __name__,
    "modeld.stdout",
    "modeld.stderr",
)

_MODELD_LOGGING_STATE: "ModeldLoggingState | None" = None


class UtcRfc3339Formatter(logging.Formatter):
    def formatTime(
        self,
        record: logging.LogRecord,
        datefmt: str | None = None,
    ) -> str:
        return (
            datetime.fromtimestamp(record.created, timezone.utc)
            .isoformat(timespec="microseconds")
            .replace("+00:00", "Z")
        )

    def format(self, record: logging.LogRecord) -> str:
        record.modeld_levelname = self._level_name(record)
        formatted = super().format(record)
        if "\n" not in formatted:
            return formatted
        prefix = (
            f"{self.formatTime(record)}  "
            f"{record.modeld_levelname:>5} {record.name}: "
        )
        lines = formatted.splitlines()
        return "\n".join([lines[0], *(prefix + line for line in lines[1:])])

    def _level_name(self, record: logging.LogRecord) -> str:
        if record.levelno == logging.WARNING:
            return "WARN"
        return record.levelname


class LineLoggingStream(io.TextIOBase):
    def __init__(self, logger: logging.Logger, level: int) -> None:
        self._logger = logger
        self._level = level
        self._buffer = ""
        self._lock = threading.RLock()

    @property
    def encoding(self) -> str:
        return "utf-8"

    def writable(self) -> bool:
        return True

    def isatty(self) -> bool:
        return False

    def write(self, text: str) -> int:
        if not isinstance(text, str):
            text = str(text)
        length = len(text)
        if not text:
            return length
        with self._lock:
            self._buffer += text.replace("\r", "\n")
            self._emit_complete_lines()
        return length

    def flush(self) -> None:
        with self._lock:
            self._emit_buffer()

    def _emit_complete_lines(self) -> None:
        while "\n" in self._buffer:
            line, self._buffer = self._buffer.split("\n", 1)
            self._emit_line(line)

    def _emit_buffer(self) -> None:
        if not self._buffer:
            return
        line = self._buffer
        self._buffer = ""
        self._emit_line(line)

    def _emit_line(self, line: str) -> None:
        message = line.strip()
        if message:
            self._logger.log(self._level, message)


class ModeldLoggingState:
    def __init__(
        self,
        *,
        handler: logging.Handler,
        logger_states: list[
            tuple[logging.Logger, int, bool, list[logging.Handler]]
        ],
        stdout: LineLoggingStream,
        stderr: LineLoggingStream,
        previous_stdout: Any,
        previous_stderr: Any,
    ) -> None:
        self.handler = handler
        self.logger_states = logger_states
        self.stdout = stdout
        self.stderr = stderr
        self.previous_stdout = previous_stdout
        self.previous_stderr = previous_stderr

    def close(self) -> None:
        self.stdout.flush()
        self.stderr.flush()
        sys.stdout = self.previous_stdout
        sys.stderr = self.previous_stderr
        for logger, level, propagate, handlers in self.logger_states:
            logger.handlers = handlers
            logger.setLevel(level)
            logger.propagate = propagate
        self.handler.close()


def configure_modeld_logging_from_env() -> ModeldLoggingState | None:
    log_path = os.environ.get("FAUNI_MODELD_LOG_PATH")
    if not log_path:
        return None
    return configure_modeld_logging(Path(log_path))


def configure_modeld_logging(
    log_path: Path,
    *,
    max_bytes: int = MODELD_LOG_MAX_BYTES,
    backup_count: int = MODELD_LOG_BACKUP_COUNT,
    startup_rollover: bool = True,
) -> ModeldLoggingState:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    if startup_rollover:
        rollover_existing_modeld_log(log_path, backup_count)

    handler = RotatingFileHandler(
        log_path,
        maxBytes=max_bytes,
        backupCount=backup_count,
        encoding="utf-8",
    )
    handler.setFormatter(
        UtcRfc3339Formatter(
            "%(asctime)s  %(modeld_levelname)5s %(name)s: %(message)s"
        )
    )
    handler.setLevel(logging.INFO)

    logger_states = [
        (
            logging.getLogger(name),
            logging.getLogger(name).level,
            logging.getLogger(name).propagate,
            list(logging.getLogger(name).handlers),
        )
        for name in MODELD_LOGGER_NAMES
    ]

    root_logger = logging.getLogger()
    root_logger.handlers = [handler]
    root_logger.setLevel(logging.INFO)
    for name in MODELD_LOGGER_NAMES[1:]:
        logger = logging.getLogger(name)
        logger.handlers = []
        logger.propagate = True
        logger.setLevel(logging.INFO)

    stdout = LineLoggingStream(logging.getLogger("modeld.stdout"), logging.INFO)
    stderr = LineLoggingStream(logging.getLogger("modeld.stderr"), logging.WARNING)
    previous_stdout = sys.stdout
    previous_stderr = sys.stderr
    sys.stdout = stdout
    sys.stderr = stderr

    logging.getLogger(__name__).info("modeld logging initialized at %s", log_path)
    return ModeldLoggingState(
        handler=handler,
        logger_states=logger_states,
        stdout=stdout,
        stderr=stderr,
        previous_stdout=previous_stdout,
        previous_stderr=previous_stderr,
    )


def rollover_existing_modeld_log(log_path: Path, backup_count: int) -> None:
    if backup_count <= 0 or not log_path.exists() or log_path.stat().st_size == 0:
        return
    for index in range(backup_count, 0, -1):
        target = numbered_log_path(log_path, index)
        if index == backup_count:
            target.unlink(missing_ok=True)
            continue
        source = numbered_log_path(log_path, index)
        if source.exists():
            source.replace(numbered_log_path(log_path, index + 1))
    log_path.replace(numbered_log_path(log_path, 1))


def numbered_log_path(log_path: Path, index: int) -> Path:
    return log_path.with_name(f"{log_path.name}.{index}")


class ModelLoadRequest(BaseModel):
    model_id: str
    model_version: str | None = None
    backend: str | None = None

    @field_validator("model_id")
    @classmethod
    def validate_model_id(cls, value: str) -> str:
        normalized = value.strip()
        if not normalized:
            raise ValueError("model_id must not be empty")
        return normalized


class ModeldRuntimeManager:
    def __init__(
        self,
        default_model: LocalSidecarModelConfig,
        catalog: dict[str, LocalSidecarModelConfig],
    ) -> None:
        self.default_model = default_model
        self.catalog = catalog
        self._lock = threading.Lock()
        self._runtimes: dict[tuple[str, str, str], EmbeddingRuntime] = {}

    @classmethod
    def from_env(cls, *, preload: bool = True) -> "ModeldRuntimeManager":
        selected_model_id = os.environ.get("EMBEDDING_MODEL_ID")
        if selected_model_id is not None and not selected_model_id.strip():
            selected_model_id = None
        active_model_id, catalog = list_local_sidecar_model_configs_from_runtime_config()
        default_model = resolve_local_sidecar_model_config_from_runtime_config(
            selected_model_id or active_model_id
        )
        manager = cls(default_model=default_model, catalog=catalog)
        if preload:
            manager.ensure_model_loaded(
                default_model.model_id,
                default_model.version,
                default_model.backend,
            )
        return manager

    def health_snapshot(self) -> dict[str, Any]:
        models = [self._runtime_summary(runtime) for runtime in self._runtimes.values()]
        loaded_models = [model for model in models if model["loaded"]]
        default_summary = self._configured_summary(self.default_model)
        for model in models:
            if (
                model["model_id"] == self.default_model.model_id
                and model["revision"] == self.default_model.version
                and model["backend"] == self.default_model.backend
            ):
                default_summary = model
                break

        status = "ok"
        if any(model.get("load_error") for model in models):
            status = "degraded"
        if self.default_model.enabled is False:
            status = "degraded"

        return {
            "runtime_kind": "local_python_modeld",
            "status": status,
            "last_probe_at": utc_now(),
            "default_model": default_summary,
            "loaded_models": loaded_models,
            "models": models,
            "diagnostics": {
                "model_id": self.default_model.model_id,
                "model_revision": self.default_model.version,
                "model_backend": self.default_model.backend,
                "model_loaded": bool(default_summary.get("loaded")),
                "loaded_model_count": len(loaded_models),
                "loaded_models": loaded_models,
            },
        }

    def capabilities_snapshot(self) -> dict[str, Any]:
        runtime = self._runtime_for_context(None, preload=False)
        payload = runtime.capabilities_snapshot()
        payload["default_model"] = self._configured_summary(self.default_model)
        payload["loaded_models"] = self.health_snapshot()["loaded_models"]
        return payload

    def embed_queries(
        self,
        queries: list[str],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._runtime_for_context(provider_context).embed_queries(
            queries,
            debug=debug,
            provider_context=provider_context,
        )

    def embed_image_queries(
        self,
        images: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._runtime_for_context(provider_context).embed_image_queries(
            images,
            debug=debug,
            provider_context=provider_context,
        )

    def embed_video_queries(
        self,
        videos: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._runtime_for_context(provider_context).embed_video_queries(
            videos,
            debug=debug,
            provider_context=provider_context,
        )

    def embed_document_queries(
        self,
        documents: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._runtime_for_context(provider_context).embed_document_queries(
            documents,
            debug=debug,
            provider_context=provider_context,
        )

    def embed_documents(
        self,
        documents: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._runtime_for_context(provider_context).embed_documents(
            documents,
            debug=debug,
            provider_context=provider_context,
        )

    def ensure_model_loaded(
        self,
        model_id: str,
        model_version: str | None = None,
        backend: str | None = None,
    ) -> dict[str, Any]:
        config = self._resolve_config(model_id, model_version, backend)
        runtime = self._runtime_for_config(config, preload=True)
        self.default_model = config
        return self._runtime_summary(runtime)

    def _runtime_for_context(
        self,
        provider_context: dict[str, Any] | None,
        *,
        preload: bool = True,
    ) -> EmbeddingRuntime:
        if provider_context is None:
            return self._runtime_for_config(self.default_model, preload=preload)
        model_id = str(provider_context.get("model_id", "")).strip()
        model_version = provider_context.get("model_version")
        if not model_id:
            return self._runtime_for_config(self.default_model, preload=preload)
        config = self._resolve_config(
            model_id,
            str(model_version).strip() if model_version is not None else None,
            None,
        )
        return self._runtime_for_config(config, preload=preload)

    def _resolve_config(
        self,
        model_id: str,
        model_version: str | None,
        backend: str | None,
    ) -> LocalSidecarModelConfig:
        config = self.catalog.get(model_id)
        if config is None:
            raise RuntimeError(f"provider.local_sidecar.models does not define model {model_id}.")
        if not config.enabled:
            raise RuntimeError(f"provider.local_sidecar.models.{model_id} is disabled.")
        if model_version and model_version != config.version:
            raise RuntimeError(
                f"provider_context requested {model_id}@{model_version}, but configured version is {config.version}."
            )
        if backend and backend != config.backend:
            raise RuntimeError(
                f"provider_context requested backend {backend} for {model_id}, but configured backend is {config.backend}."
            )
        if config.backend not in {"colqwen3_5", "qwen3_vl_embedding"}:
            raise RuntimeError(
                f"provider.local_sidecar.models.{model_id}.backend is not supported: {config.backend}."
            )
        return config

    def _runtime_for_config(
        self,
        config: LocalSidecarModelConfig,
        *,
        preload: bool,
    ) -> EmbeddingRuntime:
        key = (config.model_id, config.version, config.backend)
        with self._lock:
            runtime = self._runtimes.get(key)
            if runtime is None:
                runtime = self._create_runtime(config)
                self._runtimes[key] = runtime
        if preload:
            ensure_loaded = getattr(runtime, "_ensure_loaded", None)
            if callable(ensure_loaded):
                ensure_loaded()
        return runtime

    def _create_runtime(self, config: LocalSidecarModelConfig) -> EmbeddingRuntime:
        if config.backend == "colqwen3_5":
            return ColQwenRuntime(
                model_id=config.model_id,
                model_revision=config.version,
            )
        if config.backend == "qwen3_vl_embedding":
            return Qwen3VlEmbeddingRuntime(
                model_id=config.model_id,
                model_revision=config.version,
            )
        raise RuntimeError(
            f"provider.local_sidecar.models.{config.model_id}.backend is not supported: {config.backend}."
        )

    def _configured_summary(self, config: LocalSidecarModelConfig) -> dict[str, Any]:
        return {
            "model_id": config.model_id,
            "revision": config.version,
            "backend": config.backend,
            "loaded": False,
            "status": "configured",
            "device": None,
            "dtype": None,
            "load_error": None,
        }

    def _runtime_summary(self, runtime: EmbeddingRuntime) -> dict[str, Any]:
        capabilities = runtime.capabilities_snapshot()
        operations = capabilities.get("operations", [])
        model = {}
        if operations and isinstance(operations[0], dict):
            raw_model = operations[0].get("model")
            if isinstance(raw_model, dict):
                model = dict(raw_model)
        availability = capabilities.get("availability", {})
        if isinstance(availability, dict):
            model["load_error"] = availability.get("load_error")
        model["status"] = "loaded" if model.get("loaded") else "configured"
        return model


class LazyModeldRuntimeManager:
    def __init__(self, *, preload: bool) -> None:
        self.preload = preload
        self._lock = threading.Lock()
        self._manager: ModeldRuntimeManager | None = None

    def manager(self) -> ModeldRuntimeManager:
        if self._manager is not None:
            return self._manager
        with self._lock:
            if self._manager is None:
                self._manager = ModeldRuntimeManager.from_env(preload=self.preload)
        return self._manager

    def __getattr__(self, name: str) -> Any:
        return getattr(self.manager(), name)


def create_modeld_app(runtime: EmbeddingRuntime | None = None) -> FastAPI:
    manager = runtime or LazyModeldRuntimeManager(preload=False)
    app = create_app(runtime=manager)

    @app.post("/models/load")
    @app.post("/load")
    def load_model(request: ModelLoadRequest) -> dict[str, Any]:
        active_runtime = app.state.runtime
        if isinstance(active_runtime, LazyModeldRuntimeManager):
            active_runtime = active_runtime.manager()
        if not isinstance(active_runtime, ModeldRuntimeManager):
            raise SidecarApiError(
                status_code=503,
                code="runtime_unavailable",
                message="modeld runtime manager is unavailable.",
                details={},
            )
        try:
            model = active_runtime.ensure_model_loaded(
                request.model_id,
                request.model_version,
                request.backend,
            )
        except Exception as exc:
            raise SidecarApiError(
                status_code=503,
                code="runtime_unavailable",
                message=str(exc),
                details={"model_id": request.model_id},
            ) from exc
        return {"data": {"model": model}}

    return app


app = create_modeld_app()


def main() -> None:
    global _MODELD_LOGGING_STATE
    _MODELD_LOGGING_STATE = configure_modeld_logging_from_env()
    uvicorn_options: dict[str, Any] = {
        "host": require_env("MODELD_HOST"),
        "port": int(require_env("MODELD_PORT")),
        "reload": False,
    }
    if _MODELD_LOGGING_STATE is not None:
        uvicorn_options["log_config"] = None
    uvicorn.run("fauni_sidecar.modeld:app", **uvicorn_options)


if __name__ == "__main__":
    main()
