from __future__ import annotations

import json

import pytest

from fauni_sidecar.app import EmbedRequest, ModeldRuntimeClient, SidecarApiError, create_app
from fauni_sidecar.modeld import ModelLoadRequest, ModeldRuntimeManager, create_modeld_app
from fauni_sidecar.runtime import (
    LocalSidecarModelConfig,
    load_document_image,
    resolve_local_sidecar_model_from_runtime_config,
)


class FakeRuntime:
    def health_snapshot(self) -> dict[str, object]:
        return {
            "runtime_kind": "local_python",
            "status": "ok",
            "last_probe_at": "2026-04-06T00:00:00Z",
            "diagnostics": {
                "model_id": "fake/model",
                "model_loaded": False,
            },
        }

    def capabilities_snapshot(self) -> dict[str, object]:
        return {
            "runtime_kind": "local_python",
            "status": "ok",
            "availability": {
                "can_service": True,
                "model_loaded": False,
                "load_error": None,
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
                    "supported": True,
                    "input_kind": "text",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3_5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                },
                {
                    "operation_kind": "image_query_embedding",
                    "supported": True,
                    "input_kind": "local_file",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3_5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                },
                {
                    "operation_kind": "video_query_embedding",
                    "supported": True,
                    "input_kind": "local_file",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3_5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                },
                {
                    "operation_kind": "document_query_embedding",
                    "supported": True,
                    "input_kind": "local_file",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3_5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                },
                {
                    "operation_kind": "document_embedding",
                    "supported": True,
                    "input_kind": "local_file",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3_5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                }
            ],
        }

    def embed_queries(
        self,
        queries: list[str],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "query_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3_5",
                "loaded": True,
                "device": "cuda:0",
                "dtype": "torch.bfloat16",
            },
            "embeddings": [
                {
                    "index": index,
                    "text": text,
                    "vector_count": 2,
                    "dim": 3,
                    "vectors": [[1.0, 0.0, 0.0], [0.5, 0.5, 0.0]],
                }
                for index, text in enumerate(queries)
            ],
        }
        if debug:
            payload["debug"] = {"elapsed_ms": 1.23}
        return payload

    def embed_image_queries(
        self,
        images: list[dict[str, object]],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "image_query_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3_5",
                "loaded": True,
                "device": "cuda:0",
                "dtype": "torch.bfloat16",
            },
            "embeddings": [
                {
                    "index": index,
                    "path": image["path"],
                    "source_type": "pdf" if image.get("locator") else "image",
                    "kind": "document_page" if image.get("locator") else "image",
                    "locator": image.get("locator") or {"path": image["path"]},
                    "vector_count": 2,
                    "dim": 3,
                    "vectors": [[0.2, 0.3, 0.5], [0.1, 0.7, 0.2]],
                    "pooled_vector": [0.15, 0.5, 0.35],
                }
                for index, image in enumerate(images)
            ],
        }
        if debug:
            payload["debug"] = {"elapsed_ms": 1.89}
        return payload

    def embed_video_queries(
        self,
        videos: list[dict[str, object]],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "video_query_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3_5",
                "loaded": True,
                "device": "cuda:0",
                "dtype": "torch.bfloat16",
            },
            "embeddings": [
                {
                    "index": index,
                    "path": video["path"],
                    "source_type": "video",
                    "kind": "video",
                    "locator": video.get("locator", {"start_ms": 0, "end_ms": 5000, "duration_ms": 5000}),
                    "frame_count": 2,
                    "vector_count": 4,
                    "dim": 3,
                    "vectors": [[0.2, 0.3, 0.5], [0.1, 0.7, 0.2], [0.4, 0.1, 0.5], [0.4, 0.4, 0.2]],
                    "pooled_vector": [0.275, 0.375, 0.35],
                }
                for index, video in enumerate(videos)
            ],
        }
        if debug:
            payload["debug"] = {"elapsed_ms": 2.11}
        return payload

    def embed_document_queries(
        self,
        documents: list[dict[str, object]],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "document_query_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3_5",
                "loaded": True,
                "device": "cuda:0",
                "dtype": "torch.bfloat16",
            },
            "embeddings": [
                {
                    "index": index,
                    "path": document["path"],
                    "source_type": "pdf",
                    "kind": "document",
                    "locator": document.get("locator", {"start_page": 1, "end_page": 3}),
                    "page_count": 3,
                    "vector_count": 4,
                    "dim": 3,
                    "vectors": [[0.2, 0.3, 0.5], [0.1, 0.7, 0.2], [0.4, 0.1, 0.5], [0.4, 0.4, 0.2]],
                    "pooled_vector": [0.275, 0.375, 0.35],
                }
                for index, document in enumerate(documents)
            ],
        }
        if debug:
            payload["debug"] = {"elapsed_ms": 2.23}
        return payload

    def embed_documents(
        self,
        documents: list[dict[str, object]],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "document_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3_5",
                "loaded": True,
                "device": "cuda:0",
                "dtype": "torch.bfloat16",
            },
            "embeddings": [
                {
                    "index": index,
                    "path": document["path"],
                    "source_type": "image",
                    "kind": "image",
                    "locator": document.get("locator", {"path": document["path"]}),
                    "vector_count": 2,
                    "dim": 3,
                    "vectors": [[1.0, 0.0, 0.0], [0.5, 0.5, 0.0]],
                    "pooled_vector": [0.75, 0.25, 0.0],
                }
                for index, document in enumerate(documents)
            ],
        }
        if debug:
            payload["debug"] = {"elapsed_ms": 2.34}
        return payload


class FailingRuntime(FakeRuntime):
    def embed_queries(
        self,
        queries: list[str],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        raise RuntimeError("CUDA is unavailable in the current GPU environment.")


class RecordingRuntime(FakeRuntime):
    def __init__(self) -> None:
        self.provider_context: dict[str, object] | None = None

    def embed_queries(
        self,
        queries: list[str],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        self.provider_context = provider_context
        return super().embed_queries(queries, debug=debug, provider_context=provider_context)


class LoadableNamedRuntime(FakeRuntime):
    def __init__(self, config: LocalSidecarModelConfig) -> None:
        self.config = config
        self.loaded = False

    def _ensure_loaded(self) -> None:
        self.loaded = True

    def capabilities_snapshot(self) -> dict[str, object]:
        payload = super().capabilities_snapshot()
        for operation in payload["operations"]:  # type: ignore[index]
            operation["model"] = {  # type: ignore[index]
                "model_id": self.config.model_id,
                "revision": self.config.version,
                "backend": self.config.backend,
                "loaded": self.loaded,
                "device": "cuda:0" if self.loaded else None,
                "dtype": "torch.bfloat16" if self.loaded else None,
            }
        payload["availability"]["model_loaded"] = self.loaded  # type: ignore[index]
        return payload

    def embed_queries(
        self,
        queries: list[str],
        debug: bool = False,
        provider_context: dict[str, object] | None = None,
    ) -> dict[str, object]:
        self.loaded = True
        payload = super().embed_queries(queries, debug=debug, provider_context=provider_context)
        payload["model"] = {
            "model_id": self.config.model_id,
            "revision": self.config.version,
            "backend": self.config.backend,
            "loaded": True,
            "device": "cuda:0",
            "dtype": "torch.bfloat16",
        }
        return payload


class FakeModeldManager(ModeldRuntimeManager):
    def _create_runtime(self, config: LocalSidecarModelConfig) -> LoadableNamedRuntime:
        return LoadableNamedRuntime(config)


def model_config(
    model_id: str,
    *,
    version: str = "main",
    backend: str = "colqwen3_5",
    enabled: bool = True,
) -> LocalSidecarModelConfig:
    return LocalSidecarModelConfig(
        model_id=model_id,
        version=version,
        backend=backend,
        enabled=enabled,
        embedding_capabilities={},
    )


def build_route_map(runtime: object) -> dict[str, object]:
    app = create_app(runtime=runtime)
    return {route.path: route.endpoint for route in app.router.routes if hasattr(route, "endpoint")}


def build_modeld_route_map(runtime: object) -> dict[str, object]:
    app = create_modeld_app(runtime=runtime)
    return {route.path: route.endpoint for route in app.router.routes if hasattr(route, "endpoint")}


def test_capabilities_exposes_query_embedding_operation() -> None:
    routes = build_route_map(FakeRuntime())

    payload = routes["/capabilities"]()

    assert payload["status"] == "ok"
    assert payload["embedding_capabilities"] == {
        "input_types": ["text", "image"],
        "vector_types": ["multi_vector_late_interaction"],
        "supports_mixed_inputs": False,
    }
    assert payload["execution_input_types"] == ["text", "image", "document", "video"]
    assert payload["runtime_adapters"] == [
        "document_query_via_page_images",
        "video_query_via_frame_images",
    ]
    assert [item["operation_kind"] for item in payload["operations"]] == [
        "query_embedding",
        "image_query_embedding",
        "video_query_embedding",
        "document_query_embedding",
        "document_embedding",
    ]
    assert "target_index_lines" not in payload["operations"][0]


def test_embed_returns_query_vectors() -> None:
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "query_embedding",
            "inputs": {"queries": ["what is the revenue?"]},
            "debug": True,
        }
    )

    response = routes["/embed"](request)
    payload = response["data"]

    assert payload["operation_kind"] == "query_embedding"
    assert payload["embeddings"][0]["vector_count"] == 2
    assert payload["debug"]["elapsed_ms"] == 1.23


def test_embed_forwards_provider_context_to_runtime() -> None:
    runtime = RecordingRuntime()
    routes = build_route_map(runtime)
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "query_embedding",
            "inputs": {"queries": ["what is the revenue?"]},
            "provider_context": {
                "provider_id": "local_sidecar",
                "model_id": "Qwen/Qwen3-VL-Embedding-2B",
                "model_version": "main",
                "vector_type": "single_vector",
            },
        }
    )

    routes["/embed"](request)

    assert runtime.provider_context == {
        "provider_id": "local_sidecar",
        "model_id": "Qwen/Qwen3-VL-Embedding-2B",
        "model_version": "main",
        "vector_type": "single_vector",
    }


def test_modeld_runtime_client_forwards_provider_context() -> None:
    client = ModeldRuntimeClient(base_url="http://modeld.test")
    captured: dict[str, object] = {}

    def fake_request_json(
        method: str,
        path: str,
        *,
        json: dict[str, object] | None = None,
    ) -> dict[str, object]:
        captured["method"] = method
        captured["path"] = path
        captured["json"] = json
        return {"data": {"embeddings": []}}

    client._request_json = fake_request_json  # type: ignore[method-assign]
    provider_context = {
        "provider_id": "local_sidecar",
        "model_id": "Qwen/Qwen3-VL-Embedding-2B",
        "model_version": "main",
        "vector_type": "single_vector",
    }

    client.embed_queries(["what is the revenue?"], provider_context=provider_context)

    assert captured["method"] == "POST"
    assert captured["path"] == "/embed"
    assert captured["json"] == {
        "operation_kind": "query_embedding",
        "inputs": {"queries": ["what is the revenue?"]},
        "debug": False,
        "provider_context": provider_context,
    }


def test_embed_returns_document_vectors() -> None:
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "document_embedding",
            "inputs": {
                "documents": [
                    {"path": "/tmp/example.pdf", "locator": {"page": 2, "page_label": "2"}}
                ]
            },
            "debug": True,
        }
    )

    response = routes["/embed"](request)
    payload = response["data"]

    assert payload["operation_kind"] == "document_embedding"
    assert payload["embeddings"][0]["path"] == "/tmp/example.pdf"
    assert payload["embeddings"][0]["locator"] == {"page": 2, "page_label": "2"}
    assert payload["embeddings"][0]["pooled_vector"] == [0.75, 0.25, 0.0]
    assert payload["debug"]["elapsed_ms"] == 2.34


def test_load_document_image_preserves_explicit_image_locator(tmp_path) -> None:
    image_module = pytest.importorskip("PIL.Image")
    image_path = tmp_path / "sample.png"
    image_module.new("RGB", (1, 1), color=(255, 255, 255)).save(image_path)

    locator = {"source_uri": f"file://{image_path}"}
    _image, source_type, kind, returned_locator = load_document_image(
        {"path": str(image_path), "locator": locator}
    )

    assert source_type == "image"
    assert kind == "image"
    assert returned_locator == locator


def test_embed_rejects_document_batches_over_runtime_limit(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("INDEX_EMBED_BATCH_ITEMS", "1")
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "document_embedding",
            "inputs": {
                "documents": [
                    {"path": "/tmp/example-a.pdf"},
                    {"path": "/tmp/example-b.pdf"},
                ]
            },
        }
    )

    with pytest.raises(SidecarApiError) as excinfo:
        routes["/embed"](request)

    assert excinfo.value.status_code == 422
    assert excinfo.value.code == "validation_failed"
    assert excinfo.value.details == {
        "field": "inputs.documents",
        "limit": 1,
        "received": 2,
    }


def test_embed_returns_document_query_vectors() -> None:
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "document_query_embedding",
            "inputs": {
                "documents": [
                    {
                        "path": "/tmp/query.pdf",
                        "locator": {"start_page": 2, "end_page": 3},
                    }
                ]
            },
            "debug": True,
        }
    )

    response = routes["/embed"](request)
    payload = response["data"]

    assert payload["operation_kind"] == "document_query_embedding"
    assert payload["embeddings"][0]["path"] == "/tmp/query.pdf"
    assert payload["embeddings"][0]["locator"] == {"start_page": 2, "end_page": 3}
    assert payload["embeddings"][0]["pooled_vector"] == [0.275, 0.375, 0.35]
    assert payload["debug"]["elapsed_ms"] == 2.23


def test_embed_returns_image_query_vectors() -> None:
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "image_query_embedding",
            "inputs": {
                "images": [
                    {"path": "/tmp/query.png"}
                ]
            },
            "debug": True,
        }
    )

    response = routes["/embed"](request)
    payload = response["data"]

    assert payload["operation_kind"] == "image_query_embedding"
    assert payload["embeddings"][0]["path"] == "/tmp/query.png"
    assert payload["embeddings"][0]["locator"] == {"path": "/tmp/query.png"}
    assert payload["embeddings"][0]["pooled_vector"] == [0.15, 0.5, 0.35]
    assert payload["debug"]["elapsed_ms"] == 1.89


def test_embed_returns_document_page_query_vectors() -> None:
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "image_query_embedding",
            "inputs": {
                "images": [
                    {"path": "/tmp/query.pdf", "locator": {"page": 2, "page_label": "2"}}
                ]
            },
            "debug": True,
        }
    )

    response = routes["/embed"](request)
    payload = response["data"]

    assert payload["operation_kind"] == "image_query_embedding"
    assert payload["embeddings"][0]["path"] == "/tmp/query.pdf"
    assert payload["embeddings"][0]["kind"] == "document_page"
    assert payload["embeddings"][0]["locator"] == {"page": 2, "page_label": "2"}
    assert payload["embeddings"][0]["pooled_vector"] == [0.15, 0.5, 0.35]


def test_embed_returns_video_query_vectors() -> None:
    routes = build_route_map(FakeRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "video_query_embedding",
            "inputs": {
                "videos": [
                    {"path": "/tmp/query.mp4", "locator": {"start_ms": 1000, "end_ms": 5000}}
                ]
            },
            "debug": True,
        }
    )

    response = routes["/embed"](request)
    payload = response["data"]

    assert payload["operation_kind"] == "video_query_embedding"
    assert payload["embeddings"][0]["path"] == "/tmp/query.mp4"
    assert payload["embeddings"][0]["locator"] == {
        "start_ms": 1000,
        "end_ms": 5000,
    }
    assert payload["embeddings"][0]["frame_count"] == 2
    assert payload["embeddings"][0]["pooled_vector"] == [0.275, 0.375, 0.35]
    assert payload["debug"]["elapsed_ms"] == 2.11


def test_embed_runtime_failure_maps_to_runtime_unavailable() -> None:
    routes = build_route_map(FailingRuntime())
    request = EmbedRequest.model_validate(
        {
            "operation_kind": "query_embedding",
            "inputs": {"queries": ["what is the revenue?"]},
        }
    )

    with pytest.raises(SidecarApiError) as excinfo:
        routes["/embed"](request)

    assert excinfo.value.status_code == 503
    assert excinfo.value.code == "runtime_unavailable"
    assert "CUDA is unavailable" in excinfo.value.message


def test_modeld_manager_caches_and_loads_multiple_models() -> None:
    catalog = {
        "model-a": model_config("model-a", version="a1"),
        "model-b": model_config("model-b", version="b1", backend="qwen3_vl_embedding"),
    }
    manager = FakeModeldManager(default_model=catalog["model-a"], catalog=catalog)

    manager.ensure_model_loaded("model-a", "a1", "colqwen3_5")
    response = manager.embed_queries(
        ["hello"],
        provider_context={"model_id": "model-b", "model_version": "b1"},
    )
    health = manager.health_snapshot()

    assert response["model"]["model_id"] == "model-b"
    assert sorted(model["model_id"] for model in health["loaded_models"]) == ["model-a", "model-b"]


def test_modeld_models_load_route_loads_requested_model() -> None:
    catalog = {
        "model-a": model_config("model-a", version="a1"),
        "model-b": model_config("model-b", version="b1", backend="qwen3_vl_embedding"),
    }
    manager = FakeModeldManager(default_model=catalog["model-a"], catalog=catalog)
    routes = build_modeld_route_map(manager)

    response = routes["/models/load"](
        ModelLoadRequest(
            model_id="model-b",
            model_version="b1",
            backend="qwen3_vl_embedding",
        )
    )

    assert response["data"]["model"]["model_id"] == "model-b"
    assert response["data"]["model"]["loaded"] is True
    assert manager.health_snapshot()["default_model"]["model_id"] == "model-b"


def test_modeld_manager_rejects_unknown_disabled_and_backend_mismatch() -> None:
    catalog = {
        "model-a": model_config("model-a", version="a1"),
        "model-disabled": model_config("model-disabled", enabled=False),
    }
    manager = FakeModeldManager(default_model=catalog["model-a"], catalog=catalog)

    with pytest.raises(RuntimeError, match="does not define model"):
        manager.ensure_model_loaded("missing")
    with pytest.raises(RuntimeError, match="is disabled"):
        manager.ensure_model_loaded("model-disabled")
    with pytest.raises(RuntimeError, match="configured backend"):
        manager.ensure_model_loaded("model-a", "a1", "qwen3_vl_embedding")


def test_runtime_model_falls_back_to_merged_config(tmp_path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo_config = tmp_path / "fauni.config.json"
    runtime_dir = tmp_path / "runtime"
    runtime_dir.mkdir()

    repo_config.write_text(
        json.dumps(
            {
                "provider": {
                    "local_sidecar": {
                        "kind": "local_sidecar",
                        "active_model": "model-a",
                        "models": {
                            "model-a": {
                                "enabled": True,
                                "version": "main",
                                "embedding_capabilities": {
                                    "input_types": ["text", "image"],
                                    "vector_types": ["multi_vector_late_interaction"],
                                    "supports_mixed_inputs": False,
                                },
                            }
                        },
                    }
                }
            }
        ),
        encoding="utf-8",
    )
    (runtime_dir / "runtime-config.json").write_text(
        json.dumps(
            {
                "provider": {
                    "local_sidecar": {
                        "models": {
                            "model-a": {
                                "version": "custom-version",
                            }
                        }
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    monkeypatch.delenv("EMBEDDING_MODEL_ID", raising=False)
    monkeypatch.delenv("EMBEDDING_MODEL_REVISION", raising=False)
    monkeypatch.setenv("FAUNI_CONFIG_PATH", str(repo_config))
    monkeypatch.setenv("APP_RUNTIME_DIR", str(runtime_dir))

    model_id, model_version = resolve_local_sidecar_model_from_runtime_config()
    assert model_id == "model-a"
    assert model_version == "custom-version"
