from __future__ import annotations

import io
import importlib.util
import os
import json
import subprocess
import threading
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Protocol


class EmbeddingRuntime(Protocol):
    def health_snapshot(self) -> dict[str, Any]:
        ...

    def capabilities_snapshot(self) -> dict[str, Any]:
        ...

    def embed_queries(self, queries: list[str], debug: bool = False) -> dict[str, Any]:
        ...

    def embed_image_queries(self, images: list[dict[str, Any]], debug: bool = False) -> dict[str, Any]:
        ...

    def embed_video_queries(self, videos: list[dict[str, Any]], debug: bool = False) -> dict[str, Any]:
        ...

    def embed_document_queries(self, documents: list[dict[str, Any]], debug: bool = False) -> dict[str, Any]:
        ...

    def embed_documents(self, documents: list[dict[str, Any]], debug: bool = False) -> dict[str, Any]:
        ...


@dataclass(slots=True)
class RuntimeLoadState:
    loaded_at: str | None = None
    last_load_ms: float | None = None
    load_error: str | None = None


class ColQwenRuntime:
    def __init__(
        self,
        model_id: str,
        model_revision: str,
        *,
        attn_implementation: str = "sdpa",
        device_map: str = "cuda",
    ) -> None:
        self.model_id = model_id
        self.model_revision = model_revision
        self.attn_implementation = attn_implementation
        self.device_map = device_map

        self._load_lock = threading.Lock()
        self._model: Any | None = None
        self._processor: Any | None = None
        self._load_state = RuntimeLoadState()

    @classmethod
    def from_env(cls) -> "ColQwenRuntime":
        model_id = os.environ.get("EMBEDDING_MODEL_ID")
        model_revision = os.environ.get("EMBEDDING_MODEL_REVISION")
        if not model_id or not model_revision:
            model_id, model_revision = resolve_local_sidecar_model_from_runtime_config()
        return cls(model_id=model_id, model_revision=model_revision)

    def health_snapshot(self) -> dict[str, Any]:
        probe_at = utc_now()
        torch_state = self._torch_state()
        dependency_state = self._dependency_state()

        status = "ok"
        if not dependency_state["colpali_engine_available"] or not torch_state["cuda_available"]:
            status = "degraded"
        if self._load_state.load_error is not None:
            status = "degraded"

        return {
            "runtime_kind": "local_python",
            "status": status,
            "last_probe_at": probe_at,
            "diagnostics": {
                "model_id": self.model_id,
                "model_revision": self.model_revision,
                "model_loaded": self._model is not None,
                "attn_implementation": self.attn_implementation,
                "device_map": self.device_map,
                "hf_endpoint": os.environ.get("HF_ENDPOINT"),
                "hf_transfer_enabled": os.environ.get("HF_HUB_ENABLE_HF_TRANSFER", "0") == "1",
                "dependencies": dependency_state,
                "torch": torch_state,
                "load_state": {
                    "loaded_at": self._load_state.loaded_at,
                    "last_load_ms": self._load_state.last_load_ms,
                    "load_error": self._load_state.load_error,
                },
            },
        }

    def capabilities_snapshot(self) -> dict[str, Any]:
        health = self.health_snapshot()
        diagnostics = health["diagnostics"]
        dependencies = diagnostics["dependencies"]
        torch_state = diagnostics["torch"]
        can_service = bool(
            dependencies["colpali_engine_available"] and torch_state["cuda_available"]
        )

        return {
            "runtime_kind": "local_python",
            "status": health["status"],
            "availability": {
                "can_service": can_service,
                "model_loaded": diagnostics["model_loaded"],
                "load_error": diagnostics["load_state"]["load_error"],
            },
            "embedding_capabilities": {
                "input_types": ["text", "image"],
                "vector_types": ["multi_vector_late_interaction"],
                "supports_mixed_inputs": False,
            },
            "execution_input_types": ["text", "image", "document", "video"],
            "runtime_adapters": [
                "document_query_via_page_images",
                "video_query_via_frame_images",
            ],
            "operations": [
                {
                    "operation_kind": "query_embedding",
                    "supported": can_service,
                    "input_kind": "text",
                    "model": self._model_metadata(),
                },
                {
                    "operation_kind": "image_query_embedding",
                    "supported": can_service,
                    "input_kind": "local_file",
                    "model": self._model_metadata(),
                },
                {
                    "operation_kind": "video_query_embedding",
                    "supported": can_service,
                    "input_kind": "local_file",
                    "model": self._model_metadata(),
                },
                {
                    "operation_kind": "document_query_embedding",
                    "supported": can_service,
                    "input_kind": "local_file",
                    "model": self._model_metadata(),
                },
                {
                    "operation_kind": "document_embedding",
                    "supported": can_service,
                    "input_kind": "local_file",
                    "model": self._model_metadata(),
                }
            ],
        }

    def embed_queries(self, queries: list[str], debug: bool = False) -> dict[str, Any]:
        model, processor = self._ensure_loaded()

        import torch

        device = model.device
        batch = processor.process_queries(queries).to(device)
        started_at = time.perf_counter()

        with torch.inference_mode():
            model.rope_deltas = None
            embeddings = model(**batch)

        elapsed_ms = round((time.perf_counter() - started_at) * 1000, 2)
        attention_mask = batch["attention_mask"].bool()
        items = []

        for index, text in enumerate(queries):
            vectors = embeddings[index][attention_mask[index]].to(torch.float32).cpu()
            items.append(
                {
                    "index": index,
                    "text": text,
                    "vector_count": int(vectors.shape[0]),
                    "dim": int(vectors.shape[1]) if vectors.ndim == 2 else 0,
                    "vectors": vectors.tolist(),
                }
            )

        payload: dict[str, Any] = {
            "operation_kind": "query_embedding",
            "model": self._model_metadata(loaded=True),
            "embeddings": items,
        }

        if debug:
            payload["debug"] = {
                "elapsed_ms": elapsed_ms,
                "loaded_at": self._load_state.loaded_at,
                "last_load_ms": self._load_state.last_load_ms,
            }

        return payload

    def embed_image_queries(self, images: list[dict[str, Any]], debug: bool = False) -> dict[str, Any]:
        model, processor = self._ensure_loaded()

        import torch

        started_at = time.perf_counter()
        items = []

        for index, image_input in enumerate(images):
            path = image_input["path"]
            image, source_type, kind, locator = load_query_input_image(image_input)
            batch = processor.process_images([image]).to(model.device)

            with torch.inference_mode():
                model.rope_deltas = None
                embeddings = model(**batch)

            vectors = embeddings[0].to(torch.float32).cpu()
            pooled_vector = vectors.mean(dim=0) if vectors.ndim == 2 and vectors.shape[0] > 0 else None

            items.append(
                {
                    "index": index,
                    "path": path,
                    "source_type": source_type,
                    "kind": kind,
                    "locator": locator,
                    "vector_count": int(vectors.shape[0]),
                    "dim": int(vectors.shape[1]) if vectors.ndim == 2 else 0,
                    "vectors": vectors.tolist(),
                    "pooled_vector": pooled_vector.tolist() if pooled_vector is not None else [],
                }
            )

        elapsed_ms = round((time.perf_counter() - started_at) * 1000, 2)
        payload: dict[str, Any] = {
            "operation_kind": "image_query_embedding",
            "model": self._model_metadata(loaded=True),
            "embeddings": items,
        }

        if debug:
            payload["debug"] = {
                "elapsed_ms": elapsed_ms,
                "loaded_at": self._load_state.loaded_at,
                "last_load_ms": self._load_state.last_load_ms,
            }

        return payload

    def embed_video_queries(self, videos: list[dict[str, Any]], debug: bool = False) -> dict[str, Any]:
        model, processor = self._ensure_loaded()

        import torch

        started_at = time.perf_counter()
        items = []

        for index, video_input in enumerate(videos):
            path = video_input["path"]
            frames, source_type, kind, locator = load_query_input_video(video_input)
            batch = processor.process_images(frames).to(model.device)

            with torch.inference_mode():
                model.rope_deltas = None
                embeddings = model(**batch)

            frame_vectors = []
            for frame_index in range(len(frames)):
                vectors = embeddings[frame_index].to(torch.float32).cpu()
                frame_vectors.append(vectors)

            pooled_source = torch.cat(frame_vectors, dim=0)
            pooled_vector = (
                pooled_source.mean(dim=0)
                if pooled_source.ndim == 2 and pooled_source.shape[0] > 0
                else None
            )

            items.append(
                {
                    "index": index,
                    "path": path,
                    "source_type": source_type,
                    "kind": kind,
                    "locator": locator,
                    "frame_count": len(frames),
                    "vector_count": int(sum(vectors.shape[0] for vectors in frame_vectors)),
                    "dim": int(frame_vectors[0].shape[1]) if frame_vectors and frame_vectors[0].ndim == 2 else 0,
                    "vectors": [vector.tolist() for vectors in frame_vectors for vector in vectors],
                    "pooled_vector": pooled_vector.tolist() if pooled_vector is not None else [],
                }
            )

        elapsed_ms = round((time.perf_counter() - started_at) * 1000, 2)
        payload: dict[str, Any] = {
            "operation_kind": "video_query_embedding",
            "model": self._model_metadata(loaded=True),
            "embeddings": items,
        }

        if debug:
            payload["debug"] = {
                "elapsed_ms": elapsed_ms,
                "loaded_at": self._load_state.loaded_at,
                "last_load_ms": self._load_state.last_load_ms,
            }

        return payload

    def embed_document_queries(
        self, documents: list[dict[str, Any]], debug: bool = False
    ) -> dict[str, Any]:
        model, processor = self._ensure_loaded()

        import torch

        started_at = time.perf_counter()
        items = []

        for index, document in enumerate(documents):
            path = document["path"]
            pages, locator = load_document_query_pages(document)
            batch = processor.process_images(pages).to(model.device)

            with torch.inference_mode():
                model.rope_deltas = None
                embeddings = model(**batch)

            page_vectors = []
            for page_index in range(len(pages)):
                vectors = embeddings[page_index].to(torch.float32).cpu()
                page_vectors.append(vectors)

            pooled_source = torch.cat(page_vectors, dim=0)
            pooled_vector = (
                pooled_source.mean(dim=0)
                if pooled_source.ndim == 2 and pooled_source.shape[0] > 0
                else None
            )

            items.append(
                {
                    "index": index,
                    "path": path,
                    "source_type": "pdf",
                    "kind": "document",
                    "locator": locator,
                    "page_count": len(pages),
                    "vector_count": int(sum(vectors.shape[0] for vectors in page_vectors)),
                    "dim": int(page_vectors[0].shape[1]) if page_vectors and page_vectors[0].ndim == 2 else 0,
                    "vectors": [vector.tolist() for vectors in page_vectors for vector in vectors],
                    "pooled_vector": pooled_vector.tolist() if pooled_vector is not None else [],
                }
            )

        elapsed_ms = round((time.perf_counter() - started_at) * 1000, 2)
        payload: dict[str, Any] = {
            "operation_kind": "document_query_embedding",
            "model": self._model_metadata(loaded=True),
            "embeddings": items,
        }

        if debug:
            payload["debug"] = {
                "elapsed_ms": elapsed_ms,
                "loaded_at": self._load_state.loaded_at,
                "last_load_ms": self._load_state.last_load_ms,
            }

        return payload

    def embed_documents(
        self, documents: list[dict[str, Any]], debug: bool = False
    ) -> dict[str, Any]:
        model, processor = self._ensure_loaded()

        import torch

        started_at = time.perf_counter()
        items = []

        for index, document in enumerate(documents):
            path = document["path"]
            image, source_type, kind, locator = load_document_image(document)
            batch = processor.process_images([image]).to(model.device)

            with torch.inference_mode():
                model.rope_deltas = None
                embeddings = model(**batch)

            vectors = embeddings[0].to(torch.float32).cpu()
            pooled_vector = vectors.mean(dim=0) if vectors.ndim == 2 and vectors.shape[0] > 0 else None

            items.append(
                {
                    "index": index,
                    "path": path,
                    "source_type": source_type,
                    "kind": kind,
                    "locator": locator,
                    "vector_count": int(vectors.shape[0]),
                    "dim": int(vectors.shape[1]) if vectors.ndim == 2 else 0,
                    "vectors": vectors.tolist(),
                    "pooled_vector": pooled_vector.tolist() if pooled_vector is not None else [],
                }
            )

        elapsed_ms = round((time.perf_counter() - started_at) * 1000, 2)
        payload: dict[str, Any] = {
            "operation_kind": "document_embedding",
            "model": self._model_metadata(loaded=True),
            "embeddings": items,
        }

        if debug:
            payload["debug"] = {
                "elapsed_ms": elapsed_ms,
                "loaded_at": self._load_state.loaded_at,
                "last_load_ms": self._load_state.last_load_ms,
            }

        return payload

    def _ensure_loaded(self) -> tuple[Any, Any]:
        if self._model is not None and self._processor is not None:
            return self._model, self._processor

        with self._load_lock:
            if self._model is not None and self._processor is not None:
                return self._model, self._processor

            dependencies = self._dependency_state()
            if not dependencies["colpali_engine_available"]:
                message = "colpali_engine is unavailable in the current GPU environment."
                self._load_state.load_error = message
                raise RuntimeError(message)

            import torch
            from colpali_engine.models import ColQwen3_5, ColQwen3_5Processor

            if not torch.cuda.is_available():
                message = "CUDA is unavailable in the current GPU environment."
                self._load_state.load_error = message
                raise RuntimeError(message)

            started_at = time.perf_counter()
            try:
                model = ColQwen3_5.from_pretrained(
                    self.model_id,
                    revision=self.model_revision,
                    torch_dtype=torch.bfloat16,
                    device_map=self.device_map,
                    attn_implementation=self.attn_implementation,
                )
                processor = ColQwen3_5Processor.from_pretrained(
                    self.model_id,
                    revision=self.model_revision,
                )
                model.eval()
            except Exception as exc:  # pragma: no cover - exercised in runtime smoke, not narrow tests.
                self._load_state.load_error = f"{type(exc).__name__}: {exc}"
                raise

            self._model = model
            self._processor = processor
            self._load_state.load_error = None
            self._load_state.loaded_at = utc_now()
            self._load_state.last_load_ms = round((time.perf_counter() - started_at) * 1000, 2)
            return model, processor

    def _dependency_state(self) -> dict[str, bool]:
        return {
            "colpali_engine_available": module_available("colpali_engine"),
            "torch_available": module_available("torch"),
        }

    def _model_metadata(self, *, loaded: bool | None = None) -> dict[str, Any]:
        if loaded is None:
            loaded = self._model is not None

        device = None
        dtype = None
        if self._model is not None:
            device = str(self._model.device)
            dtype = str(getattr(self._model, "dtype", "unknown"))

        return {
            "model_id": self.model_id,
            "revision": self.model_revision,
            "backend": "colqwen3.5",
            "loaded": loaded,
            "device": device,
            "dtype": dtype,
        }

    def _torch_state(self) -> dict[str, Any]:
        if not module_available("torch"):
            return {
                "cuda_available": False,
                "device_count": 0,
                "device_name": None,
                "version": None,
                "cuda_version": None,
            }

        import torch

        cuda_available = bool(torch.cuda.is_available())
        device_count = torch.cuda.device_count() if cuda_available else 0
        device_name = torch.cuda.get_device_name(0) if cuda_available and device_count > 0 else None
        return {
            "cuda_available": cuda_available,
            "device_count": device_count,
            "device_name": device_name,
            "version": torch.__version__,
            "cuda_version": torch.version.cuda,
        }


def module_available(name: str) -> bool:
    return importlib.util.find_spec(name) is not None


def resolve_local_sidecar_model_from_runtime_config() -> tuple[str, str]:
    config_path = os.environ.get("FAUNI_CONFIG_PATH")
    repo_path = Path(config_path) if config_path else Path.cwd() / "fauni.config.json"
    runtime_dir = os.environ.get("APP_RUNTIME_DIR")
    if not runtime_dir:
        raise RuntimeError(
            "Missing required environment variable APP_RUNTIME_DIR; source .env or use scripts/local/run.sh"
        )

    repo_config = load_json_config(repo_path, required=True)
    runtime_config = load_json_config(Path(runtime_dir) / "runtime-config.json", required=False)
    merged = deep_merge_config(repo_config, runtime_config)
    providers = merged.get("provider")
    if not isinstance(providers, dict):
        raise RuntimeError("Fauni config must define provider.local_sidecar.")
    provider = providers.get("local_sidecar")
    if not isinstance(provider, dict):
        raise RuntimeError("Fauni config must define provider.local_sidecar.")
    active_model = str(provider.get("active_model", "")).strip()
    if not active_model:
        raise RuntimeError("provider.local_sidecar.active_model must be a non-empty string.")
    models = provider.get("models")
    if not isinstance(models, dict):
        raise RuntimeError("provider.local_sidecar.models must be an object.")
    model = models.get(active_model)
    if not isinstance(model, dict):
        raise RuntimeError(
            f"provider.local_sidecar.active_model points to missing model {active_model}."
        )
    version = str(model.get("version", "main")).strip() or "main"
    return active_model, version


def load_json_config(path: Path, *, required: bool) -> dict[str, Any]:
    if not path.exists():
        if required:
            raise RuntimeError(f"Fauni config file was not found: {path}")
        return {}
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"Fauni config file must decode to an object: {path}")
    return payload


def deep_merge_config(base: dict[str, Any], overlay: dict[str, Any]) -> dict[str, Any]:
    merged = dict(base)
    for key, value in overlay.items():
        if isinstance(value, dict) and isinstance(merged.get(key), dict):
            merged[key] = deep_merge_config(merged[key], value)
        else:
            merged[key] = value
    return merged


def load_document_image(document: dict[str, Any]) -> tuple[Any, str, str, dict[str, Any]]:
    path = document["path"]
    locator = document.get("locator")
    normalized = Path(path).expanduser()
    suffix = normalized.suffix.lower()

    if suffix == ".pdf":
        if not module_available("pypdfium2"):
            raise RuntimeError("pypdfium2 is unavailable in the current GPU environment.")
        import pypdfium2 as pdfium

        try:
            pdf_document = pdfium.PdfDocument(str(normalized))
            page_count = len(pdf_document)
            if page_count == 0:
                raise RuntimeError(f"PDF has no pages: {normalized}")
            page_number = resolve_pdf_page_number(locator, page_count, normalized)
            page = pdf_document.get_page(page_number - 1)
            bitmap = page.render(scale=2)
            image = bitmap.to_pil().convert("RGB")
            page.close()
            pdf_document.close()
        except Exception as exc:
            raise RuntimeError(f"Failed to render page from {normalized}: {exc}") from exc
        return (
            image,
            "pdf",
            "document_page",
            {
                "page": page_number,
                "page_label": str(page_number),
            },
        )

    if suffix in {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".gif"}:
        if not module_available("PIL"):
            raise RuntimeError("Pillow is unavailable in the current GPU environment.")
        from PIL import Image

        try:
            with Image.open(normalized) as image:
                return (
                    image.convert("RGB"),
                    "image",
                    "image",
                    {
                        "path": str(normalized),
                    },
                )
        except Exception as exc:
            raise RuntimeError(f"Failed to load image {normalized}: {exc}") from exc

    if suffix in supported_video_suffixes():
        duration_ms = probe_video_duration_ms(normalized)
        video_range = resolve_video_locator(locator, duration_ms, normalized)
        midpoint_ms = video_range["start_ms"] + ((video_range["end_ms"] - video_range["start_ms"]) // 2)
        image = extract_video_frame(normalized, midpoint_ms)
        return (
            image,
            "video",
            "video_segment",
            video_range,
        )

    raise RuntimeError(f"Unsupported document input type for embedding: {normalized}")


def load_query_input_image(image_input: dict[str, Any]) -> tuple[Any, str, str, dict[str, Any]]:
    try:
        return load_document_image(image_input)
    except RuntimeError as exc:
        raise RuntimeError(f"Failed to load query image input {image_input['path']}: {exc}") from exc


def load_query_input_video(video_input: dict[str, Any]) -> tuple[list[Any], str, str, dict[str, Any]]:
    path = video_input["path"]
    normalized = Path(path).expanduser()
    suffix = normalized.suffix.lower()
    if suffix not in supported_video_suffixes():
        raise RuntimeError(f"Unsupported query video input type for embedding: {normalized}")
    if not module_available("PIL"):
        raise RuntimeError("Pillow is unavailable in the current GPU environment.")

    duration_ms = probe_video_duration_ms(normalized)
    video_range = resolve_video_locator(video_input.get("locator"), duration_ms, normalized)
    frame_times = sample_video_query_frame_times(
        video_range["start_ms"],
        video_range["end_ms"],
    )
    frames = [extract_video_frame(normalized, time_ms) for time_ms in frame_times]
    return frames, "video", "video", video_range


def load_document_query_pages(document: dict[str, Any]) -> tuple[list[Any], dict[str, Any]]:
    path = document["path"]
    locator = document.get("locator")
    normalized = Path(path).expanduser()
    suffix = normalized.suffix.lower()
    if suffix != ".pdf":
        raise RuntimeError(
            f"Unsupported document query input type for embedding: {normalized}"
        )
    if not module_available("pypdfium2"):
        raise RuntimeError("pypdfium2 is unavailable in the current GPU environment.")

    import pypdfium2 as pdfium

    try:
        pdf_document = pdfium.PdfDocument(str(normalized))
        page_count = len(pdf_document)
        if page_count == 0:
            raise RuntimeError(f"PDF has no pages: {normalized}")
        start_page, end_page = resolve_pdf_page_range(locator, page_count, normalized)
        images = []
        for page_number in range(start_page, end_page + 1):
            page = pdf_document.get_page(page_number - 1)
            bitmap = page.render(scale=2)
            images.append(bitmap.to_pil().convert("RGB"))
            page.close()
        pdf_document.close()
    except Exception as exc:
        raise RuntimeError(f"Failed to render pages from {normalized}: {exc}") from exc

    return (
        images,
        {
            "start_page": start_page,
            "end_page": end_page,
            "page_count": page_count,
        },
    )


def resolve_pdf_page_number(
    locator: dict[str, Any] | None,
    page_count: int,
    normalized_path: Path,
) -> int:
    if locator is None:
        return 1

    raw_page = locator.get("page")
    if raw_page is None:
        raise RuntimeError(f"PDF locator for {normalized_path} is missing page.")
    if not isinstance(raw_page, int):
        raise RuntimeError(f"PDF locator for {normalized_path} must use an integer page.")
    if raw_page < 1 or raw_page > page_count:
        raise RuntimeError(
            f"PDF locator for {normalized_path} requested page {raw_page}, but the document has {page_count} page(s)."
        )
    return raw_page


def resolve_pdf_page_range(
    locator: dict[str, Any] | None,
    page_count: int,
    normalized_path: Path,
) -> tuple[int, int]:
    if locator is None:
        return 1, page_count

    raw_start_page = locator.get("start_page")
    raw_end_page = locator.get("end_page")
    if raw_start_page is None or raw_end_page is None:
        raise RuntimeError(
            f"PDF locator for {normalized_path} must include start_page and end_page."
        )
    if not isinstance(raw_start_page, int) or not isinstance(raw_end_page, int):
        raise RuntimeError(
            f"PDF locator for {normalized_path} must use integer start_page and end_page."
        )
    if raw_start_page < 1 or raw_end_page < raw_start_page or raw_end_page > page_count:
        raise RuntimeError(
            f"PDF locator for {normalized_path} must satisfy 1 <= start_page <= end_page <= {page_count}."
        )
    return raw_start_page, raw_end_page


def supported_video_suffixes() -> set[str]:
    return {".mp4", ".mov", ".m4v"}


def probe_video_duration_ms(path: Path) -> int:
    result = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        stderr = result.stderr.strip() or "unknown ffprobe error"
        raise RuntimeError(f"Failed to probe video duration for {path}: {stderr}")
    duration_text = result.stdout.strip()
    try:
        duration_seconds = float(duration_text)
    except ValueError as exc:
        raise RuntimeError(
            f"Failed to parse video duration for {path}: {duration_text!r}"
        ) from exc
    duration_ms = max(int(round(duration_seconds * 1000)), 1)
    return duration_ms


def resolve_video_locator(
    locator: dict[str, Any] | None,
    duration_ms: int,
    normalized_path: Path,
) -> dict[str, Any]:
    if locator is None:
        return {
            "start_ms": 0,
            "end_ms": duration_ms,
            "duration_ms": duration_ms,
        }

    raw_start_ms = locator.get("start_ms")
    raw_end_ms = locator.get("end_ms")
    if raw_start_ms is None or raw_end_ms is None:
        raise RuntimeError(
            f"Video locator for {normalized_path} must include start_ms and end_ms."
        )
    if not isinstance(raw_start_ms, int) or not isinstance(raw_end_ms, int):
        raise RuntimeError(
            f"Video locator for {normalized_path} must use integer start_ms and end_ms."
        )
    if raw_start_ms < 0 or raw_end_ms <= raw_start_ms or raw_end_ms > duration_ms:
        raise RuntimeError(
            f"Video locator for {normalized_path} must satisfy 0 <= start_ms < end_ms <= {duration_ms}."
        )
    return {
        "start_ms": raw_start_ms,
        "end_ms": raw_end_ms,
        "duration_ms": duration_ms,
    }


def sample_video_query_frame_times(start_ms: int, end_ms: int) -> list[int]:
    span_ms = max(end_ms - start_ms, 1)
    if span_ms < 1500:
        frame_count = 1
    elif span_ms < 8000:
        frame_count = 2
    else:
        frame_count = 4

    return [
        start_ms + int(round(span_ms * ((index + 0.5) / frame_count)))
        for index in range(frame_count)
    ]


def extract_video_frame(path: Path, time_ms: int) -> Any:
    if not module_available("PIL"):
        raise RuntimeError("Pillow is unavailable in the current GPU environment.")
    from PIL import Image

    seconds = max(time_ms / 1000.0, 0.0)
    result = subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-ss",
            f"{seconds:.3f}",
            "-i",
            str(path),
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "pipe:1",
        ],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0 or not result.stdout:
        stderr = result.stderr.decode("utf-8", errors="replace").strip() or "unknown ffmpeg error"
        raise RuntimeError(f"Failed to extract frame at {time_ms}ms from {path}: {stderr}")

    try:
        with Image.open(io.BytesIO(result.stdout)) as image:
            return image.convert("RGB")
    except Exception as exc:
        raise RuntimeError(
            f"Failed to decode extracted frame at {time_ms}ms from {path}: {exc}"
        ) from exc


def require_env(name: str) -> str:
    value = os.environ.get(name)
    if value:
        return value
    raise RuntimeError(
        f"Missing required environment variable {name}; source .env or use scripts/local/run.sh"
    )


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
