from __future__ import annotations

import pytest

from fauni_sidecar.app import EmbedRequest, SidecarApiError, create_app


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
            "operations": [
                {
                    "operation_kind": "query_embedding",
                    "supported": True,
                    "target_index_lines": ["multivector"],
                    "input_kind": "text",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3.5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                },
                {
                    "operation_kind": "image_query_embedding",
                    "supported": True,
                    "target_index_lines": ["multivector"],
                    "input_kind": "local_file",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3.5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                },
                {
                    "operation_kind": "document_embedding",
                    "supported": True,
                    "target_index_lines": ["multivector"],
                    "input_kind": "local_file",
                    "model": {
                        "model_id": "fake/model",
                        "revision": "main",
                        "backend": "colqwen3.5",
                        "loaded": False,
                        "device": None,
                        "dtype": None,
                    },
                }
            ],
        }

    def embed_queries(self, queries: list[str], debug: bool = False) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "query_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3.5",
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

    def embed_image_queries(self, images: list[dict[str, object]], debug: bool = False) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "image_query_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3.5",
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

    def embed_documents(self, documents: list[dict[str, object]], debug: bool = False) -> dict[str, object]:
        payload: dict[str, object] = {
            "operation_kind": "document_embedding",
            "model": {
                "model_id": "fake/model",
                "revision": "main",
                "backend": "colqwen3.5",
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
    def embed_queries(self, queries: list[str], debug: bool = False) -> dict[str, object]:
        raise RuntimeError("CUDA is unavailable in the current GPU environment.")


def build_route_map(runtime: object) -> dict[str, object]:
    app = create_app(runtime=runtime)
    return {route.path: route.endpoint for route in app.router.routes if hasattr(route, "endpoint")}


def test_capabilities_exposes_query_embedding_operation() -> None:
    routes = build_route_map(FakeRuntime())

    payload = routes["/capabilities"]()

    assert payload["status"] == "ok"
    assert [item["operation_kind"] for item in payload["operations"]] == [
        "query_embedding",
        "image_query_embedding",
        "document_embedding",
    ]
    assert payload["operations"][0]["target_index_lines"] == ["multivector"]


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
