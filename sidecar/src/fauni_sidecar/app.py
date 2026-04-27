from __future__ import annotations

import os
from typing import Any, Literal

import httpx
import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field, field_validator

from fauni_sidecar.runtime import EmbeddingRuntime, require_env


class SidecarApiError(Exception):
    def __init__(self, status_code: int, code: str, message: str, details: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.code = code
        self.message = message
        self.details = details


class ModeldRuntimeClient:
    def __init__(self, base_url: str | None = None) -> None:
        self.base_url = (base_url or modeld_base_url()).rstrip("/")

    def health_snapshot(self) -> dict[str, Any]:
        try:
            return self._get_json("/health")
        except RuntimeError as exc:
            return {
                "runtime_kind": "local_python",
                "status": "degraded",
                "last_probe_at": None,
                "diagnostics": {
                    "modeld_url": self.base_url,
                    "adapter": "sidecar_to_modeld",
                    "error": str(exc),
                },
            }

    def capabilities_snapshot(self) -> dict[str, Any]:
        try:
            return self._get_json("/capabilities")
        except RuntimeError as exc:
            backend = os.environ.get("EMBEDDING_MODEL_BACKEND", "colqwen3_5")
            if backend == "qwen3_vl_embedding":
                embedding_capabilities = {
                    "input_types": ["text", "image", "video"],
                    "vector_types": ["single_vector"],
                    "supports_mixed_inputs": True,
                }
            else:
                embedding_capabilities = {
                    "input_types": ["text", "image"],
                    "vector_types": ["multi_vector_late_interaction"],
                    "supports_mixed_inputs": False,
                }
            model = {
                "model_id": os.environ.get("EMBEDDING_MODEL_ID"),
                "revision": os.environ.get("EMBEDDING_MODEL_REVISION"),
                "backend": backend,
                "loaded": False,
                "device": None,
                "dtype": None,
            }
            return {
                "runtime_kind": "local_python",
                "status": "degraded",
                "availability": {
                    "can_service": False,
                    "model_loaded": False,
                    "load_error": str(exc),
                },
                "embedding_capabilities": embedding_capabilities,
                "execution_input_types": ["text", "image", "document", "video"],
                "runtime_adapters": [
                    "document_query_via_page_images",
                    "video_query_via_frame_images",
                ],
                "operations": [
                    {
                        "operation_kind": operation_kind,
                        "supported": False,
                        "input_kind": input_kind,
                        "model": model,
                    }
                    for operation_kind, input_kind in [
                        ("query_embedding", "text"),
                        ("image_query_embedding", "local_file"),
                        ("video_query_embedding", "local_file"),
                        ("document_query_embedding", "local_file"),
                        ("document_embedding", "local_file"),
                    ]
                ],
            }

    def embed_queries(
        self,
        queries: list[str],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._post_embed(
            "query_embedding",
            {"queries": queries},
            debug=debug,
            provider_context=provider_context,
        )

    def embed_image_queries(
        self,
        images: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._post_embed(
            "image_query_embedding",
            {"images": images},
            debug=debug,
            provider_context=provider_context,
        )

    def embed_video_queries(
        self,
        videos: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._post_embed(
            "video_query_embedding",
            {"videos": videos},
            debug=debug,
            provider_context=provider_context,
        )

    def embed_document_queries(
        self,
        documents: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._post_embed(
            "document_query_embedding",
            {"documents": documents},
            debug=debug,
            provider_context=provider_context,
        )

    def embed_documents(
        self,
        documents: list[dict[str, Any]],
        debug: bool = False,
        provider_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._post_embed(
            "document_embedding",
            {"documents": documents},
            debug=debug,
            provider_context=provider_context,
        )

    def _get_json(self, path: str) -> dict[str, Any]:
        return self._request_json("GET", path)

    def _post_embed(
        self,
        operation_kind: str,
        inputs: dict[str, Any],
        *,
        debug: bool,
        provider_context: dict[str, Any] | None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "operation_kind": operation_kind,
            "inputs": inputs,
            "debug": debug,
        }
        if provider_context is not None:
            payload["provider_context"] = provider_context
        envelope = self._request_json(
            "POST",
            "/embed",
            json=payload,
        )
        data = envelope.get("data")
        if not isinstance(data, dict):
            raise RuntimeError("modeld /embed response did not include a data object")
        return data

    def _request_json(
        self,
        method: str,
        path: str,
        *,
        json: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        url = f"{self.base_url}{path}"
        try:
            with httpx.Client(timeout=300.0) as client:
                response = client.request(method, url, json=json)
        except httpx.HTTPError as exc:
            raise RuntimeError(f"modeld request failed at {url}: {exc}") from exc

        try:
            payload = response.json()
        except ValueError as exc:
            raise RuntimeError(f"modeld response from {url} was not JSON") from exc

        if response.status_code < 200 or response.status_code >= 300:
            error = payload.get("error") if isinstance(payload, dict) else None
            if isinstance(error, dict):
                code = error.get("code", "modeld_error")
                message = error.get("message", f"modeld returned HTTP {response.status_code}")
                raise RuntimeError(f"modeld {code}: {message}")
            raise RuntimeError(f"modeld returned HTTP {response.status_code} at {url}")

        if not isinstance(payload, dict):
            raise RuntimeError(f"modeld response from {url} was not a JSON object")
        return payload


def modeld_base_url() -> str:
    return f"http://{require_env('MODELD_HOST')}:{require_env('MODELD_PORT')}"


class EmbedInputs(BaseModel):
    queries: list[str] | None = None
    images: list["EmbedImageInput"] | None = None
    videos: list["EmbedVideoInput"] | None = None
    documents: list["EmbedDocumentInput"] | None = None

    @field_validator("queries")
    @classmethod
    def validate_queries(cls, value: list[str] | None) -> list[str] | None:
        if value is None:
            return value
        normalized = [item.strip() for item in value]
        if any(not item for item in normalized):
            raise ValueError("queries must not contain empty text items")
        return normalized


class EmbedDocumentInput(BaseModel):
    path: str
    locator: dict[str, Any] | None = None

    @field_validator("path")
    @classmethod
    def validate_path(cls, value: str) -> str:
        normalized = value.strip()
        if not normalized:
            raise ValueError("path must not be empty")
        return normalized


class EmbedImageInput(BaseModel):
    path: str
    locator: dict[str, Any] | None = None

    @field_validator("path")
    @classmethod
    def validate_path(cls, value: str) -> str:
        normalized = value.strip()
        if not normalized:
            raise ValueError("path must not be empty")
        return normalized


class EmbedVideoInput(BaseModel):
    path: str
    locator: dict[str, Any] | None = None

    @field_validator("path")
    @classmethod
    def validate_path(cls, value: str) -> str:
        normalized = value.strip()
        if not normalized:
            raise ValueError("path must not be empty")
        return normalized


class EmbedRequest(BaseModel):
    operation_kind: Literal[
        "query_embedding",
        "image_query_embedding",
        "video_query_embedding",
        "document_query_embedding",
        "document_embedding",
    ]
    inputs: EmbedInputs
    provider_context: dict[str, Any] | None = None
    target: dict[str, Any] | None = None
    debug: bool = False


def document_embed_batch_limit() -> int:
    value = os.getenv("INDEX_EMBED_BATCH_ITEMS", "8").strip()
    try:
        parsed = int(value)
    except ValueError:
        return 8
    return max(parsed, 1)


def create_app(runtime: EmbeddingRuntime | None = None) -> FastAPI:
    app = FastAPI(title="FauniSearch Sidecar", version="0.1.0")
    app.state.runtime = runtime

    @app.exception_handler(SidecarApiError)
    async def handle_sidecar_api_error(
        request: Request, exc: SidecarApiError
    ) -> JSONResponse:
        return JSONResponse(
            status_code=exc.status_code,
            content={
                "error": {
                    "code": exc.code,
                    "message": exc.message,
                    "details": exc.details,
                }
            },
        )

    @app.get("/")
    def root() -> dict[str, object]:
        active_runtime = get_runtime(app)
        capabilities = active_runtime.capabilities_snapshot()
        return {
            "name": "fauni-sidecar",
            "status": capabilities["status"],
            "operations": [item["operation_kind"] for item in capabilities["operations"]],
        }

    @app.get("/health")
    def health() -> dict[str, object]:
        return get_runtime(app).health_snapshot()

    @app.get("/capabilities")
    def capabilities() -> dict[str, object]:
        return get_runtime(app).capabilities_snapshot()

    @app.post("/embed")
    def embed(request: EmbedRequest) -> dict[str, object]:
        runtime = get_runtime(app)
        try:
            if request.operation_kind == "query_embedding":
                if not request.inputs.queries:
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="query_embedding requires inputs.queries.",
                        details={"field": "inputs.queries"},
                    )
                data = runtime.embed_queries(
                    request.inputs.queries,
                    debug=request.debug,
                    provider_context=request.provider_context,
                )
            elif request.operation_kind == "image_query_embedding":
                if not request.inputs.images:
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="image_query_embedding requires inputs.images.",
                        details={"field": "inputs.images"},
                    )
                data = runtime.embed_image_queries(
                    [item.model_dump() for item in request.inputs.images],
                    debug=request.debug,
                    provider_context=request.provider_context,
                )
            elif request.operation_kind == "video_query_embedding":
                if not request.inputs.videos:
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="video_query_embedding requires inputs.videos.",
                        details={"field": "inputs.videos"},
                    )
                data = runtime.embed_video_queries(
                    [item.model_dump() for item in request.inputs.videos],
                    debug=request.debug,
                    provider_context=request.provider_context,
                )
            elif request.operation_kind == "document_query_embedding":
                if not request.inputs.documents:
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="document_query_embedding requires inputs.documents.",
                        details={"field": "inputs.documents"},
                    )
                data = runtime.embed_document_queries(
                    [item.model_dump() for item in request.inputs.documents],
                    debug=request.debug,
                    provider_context=request.provider_context,
                )
            else:
                if not request.inputs.documents:
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="document_embedding requires inputs.documents.",
                        details={"field": "inputs.documents"},
                    )
                if len(request.inputs.documents) > document_embed_batch_limit():
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="document_embedding batch size exceeds the current runtime limit.",
                        details={
                            "field": "inputs.documents",
                            "limit": document_embed_batch_limit(),
                            "received": len(request.inputs.documents),
                        },
                    )
                data = runtime.embed_documents(
                    [item.model_dump() for item in request.inputs.documents],
                    debug=request.debug,
                    provider_context=request.provider_context,
                )
        except SidecarApiError:
            raise
        except Exception as exc:
            raise SidecarApiError(
                status_code=503,
                code="runtime_unavailable",
                message=str(exc),
                details={"operation_kind": request.operation_kind},
            ) from exc

        return {"data": data}

    return app


def get_runtime(app: FastAPI) -> EmbeddingRuntime:
    runtime = getattr(app.state, "runtime", None)
    if runtime is None:
        runtime = ModeldRuntimeClient()
        app.state.runtime = runtime
    return runtime


app = create_app()


def main() -> None:
    uvicorn.run(
        "fauni_sidecar.app:app",
        host=require_env("SIDECAR_HOST"),
        port=int(require_env("SIDECAR_PORT")),
        reload=False,
    )
