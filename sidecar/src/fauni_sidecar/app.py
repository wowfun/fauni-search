from __future__ import annotations

from typing import Any, Literal

import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field, field_validator

from fauni_sidecar.runtime import ColQwenRuntime, EmbeddingRuntime, require_env


class SidecarApiError(Exception):
    def __init__(self, status_code: int, code: str, message: str, details: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.code = code
        self.message = message
        self.details = details


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
        "document_embedding",
    ]
    inputs: EmbedInputs
    provider_context: dict[str, Any] | None = None
    target: dict[str, Any] | None = None
    debug: bool = False


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
                data = runtime.embed_queries(request.inputs.queries, debug=request.debug)
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
                )
            else:
                if not request.inputs.documents:
                    raise SidecarApiError(
                        status_code=422,
                        code="validation_failed",
                        message="document_embedding requires inputs.documents.",
                        details={"field": "inputs.documents"},
                    )
                data = runtime.embed_documents(
                    [item.model_dump() for item in request.inputs.documents],
                    debug=request.debug,
                )
        except RuntimeError as exc:
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
        runtime = ColQwenRuntime.from_env()
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
