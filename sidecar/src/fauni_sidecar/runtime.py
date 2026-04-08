from __future__ import annotations

import importlib.util
import os
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
        model_id = require_env("TEXT_SEARCH_MODEL_ID")
        model_revision = require_env("TEXT_SEARCH_MODEL_REVISION")
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
            "operations": [
                {
                    "operation_kind": "query_embedding",
                    "supported": can_service,
                    "target_index_lines": ["multivector"],
                    "input_kind": "text",
                    "model": self._model_metadata(),
                },
                {
                    "operation_kind": "document_embedding",
                    "supported": can_service,
                    "target_index_lines": ["multivector"],
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

    raise RuntimeError(f"Unsupported document input type for embedding: {normalized}")


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


def require_env(name: str) -> str:
    value = os.environ.get(name)
    if value:
        return value
    raise RuntimeError(
        f"Missing required environment variable {name}; source .env or use scripts/local/run.sh"
    )


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
