#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import time
import urllib.request
from pathlib import Path
from urllib.error import HTTPError, URLError


ROOT = Path.cwd()
APP_URL = f"http://{os.environ['APP_HOST']}:{os.environ['APP_PORT']}"
SIDECAR_URL = f"http://{os.environ['SIDECAR_HOST']}:{os.environ['SIDECAR_PORT']}"
QDRANT_URL = os.environ["QDRANT_URL"].rstrip("/")
JOB_POLL_INTERVAL_SECONDS = 1.0
JOB_POLL_TIMEOUT_SECONDS = 300.0
LOCAL_MODEL_ID = "athrael-soju/colqwen3.5-4.5B-v3"


def get_json(url: str, timeout: int = 30) -> dict:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except URLError as exc:
        raise SystemExit(f"[error] GET {url} failed: {exc}") from exc


def post_json(url: str, payload: dict, timeout: int = 600) -> tuple[int, dict]:
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except HTTPError as exc:
        body = exc.read().decode("utf-8")
        try:
            parsed = json.loads(body)
        except json.JSONDecodeError:
            parsed = {"raw": body}
        return exc.code, parsed
    except URLError as exc:
        raise SystemExit(f"[error] POST {url} failed: {exc}") from exc


def assert_success(status: int, payload: dict, label: str) -> dict:
    if status < 200 or status >= 300:
        raise SystemExit(f"[error] {label} failed with HTTP {status}: {json.dumps(payload, ensure_ascii=False)}")
    if "data" not in payload:
        raise SystemExit(f"[error] {label} did not return a data envelope: {json.dumps(payload, ensure_ascii=False)}")
    return payload["data"]


def wait_for_job_terminal(job_id: str) -> dict:
    deadline = time.monotonic() + JOB_POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        job = get_json(f"{APP_URL}/jobs/{job_id}")["data"]
        if job["status"] in {"completed", "failed", "canceled"}:
            return job
        time.sleep(JOB_POLL_INTERVAL_SECONDS)
    raise SystemExit(f"[error] job {job_id} did not reach a terminal state in time")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print compact machine-readable JSON")
    args = parser.parse_args()

    get_json(f"{APP_URL}/health")
    get_json(f"{SIDECAR_URL}/capabilities")
    get_json(f"{QDRANT_URL}/collections")

    fixture_image = (ROOT / "tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png").resolve()
    fixture_document = (ROOT / "data/example/2025年中期报告.pdf").resolve()
    fixture_video = (ROOT / "data/example/generate_q2_report_from_csv_bank_data-720-512.mp4").resolve()

    create_status, created_payload = post_json(
        f"{APP_URL}/libraries",
        {
            "display_name": "smoke-runtime-health",
            "library_id": f"smoke-runtime-health-{int(time.time())}",
            "config": {},
        },
    )
    library = assert_success(create_status, created_payload, "create library")
    library_id = library["id"]

    import_status, imported_payload = post_json(
        f"{APP_URL}/libraries/{library_id}/imports",
        {"paths": [str(fixture_image), str(fixture_document), str(fixture_video)]},
    )
    imported = assert_success(import_status, imported_payload, "import paths")
    queued_job = imported.get("job") or {}
    job_id = queued_job.get("job_id")
    if not job_id:
        raise SystemExit(
            "[error] import did not return a queued job handle: "
            + json.dumps(imported, ensure_ascii=False)
        )

    job = wait_for_job_terminal(job_id)
    if job.get("status") != "completed":
        raise SystemExit(
            "[error] runtime-health smoke import did not complete: " + json.dumps(job, ensure_ascii=False)
        )

    runtime_health = get_json(f"{APP_URL}/runtime-health")["data"]
    resolved_models = get_json(f"{APP_URL}/libraries/{library_id}/resolved-content-models")["data"]
    diagnostics = get_json(f"{APP_URL}/libraries/{library_id}/vector-space-diagnostics")["data"]

    providers = {provider["provider_id"]: provider for provider in runtime_health.get("providers", [])}
    local_sidecar = providers.get("local_sidecar")
    if not local_sidecar:
        raise SystemExit("[error] runtime-health did not include local_sidecar diagnostics")

    if local_sidecar.get("model_id") != LOCAL_MODEL_ID:
        raise SystemExit(
            "[error] runtime-health did not report the expected local model: "
            + json.dumps(local_sidecar, ensure_ascii=False)
        )

    embedding_capabilities = local_sidecar.get("embedding_capabilities") or {}
    if embedding_capabilities.get("input_types") != ["text", "image"]:
        raise SystemExit(
            "[error] local_sidecar native embedding capabilities changed unexpectedly: "
            + json.dumps(local_sidecar, ensure_ascii=False)
        )

    execution_input_types = local_sidecar.get("execution_input_types") or []
    if execution_input_types != ["text", "image", "document", "video"]:
        raise SystemExit(
            "[error] local_sidecar execution input types are incomplete: "
            + json.dumps(local_sidecar, ensure_ascii=False)
        )

    runtime_adapters = set(local_sidecar.get("runtime_adapters") or [])
    expected_adapters = {"document_query_via_page_images", "video_query_via_frame_images"}
    if expected_adapters - runtime_adapters:
        raise SystemExit(
            "[error] runtime-health did not report the expected runtime adapters: "
            + json.dumps(local_sidecar, ensure_ascii=False)
        )

    resolved_content_types = resolved_models.get("content_types") or {}
    for content_type in ["image", "document", "video"]:
        selection = resolved_content_types.get(content_type)
        if not selection or selection.get("model_id") != LOCAL_MODEL_ID:
            raise SystemExit(
                f"[error] resolved content model for {content_type} did not point to the local ColQwen model: "
                + json.dumps(resolved_models, ensure_ascii=False)
            )

    vector_spaces = diagnostics.get("vector_spaces") or []
    active_spaces = [space for space in vector_spaces if space.get("lifecycle_state") == "active"]
    if not active_spaces:
        raise SystemExit(
            "[error] vector-space diagnostics did not report any active spaces: "
            + json.dumps(diagnostics, ensure_ascii=False)
        )
    if not any(
        space.get("model_id") == LOCAL_MODEL_ID
        and space.get("vector_type") == "multi_vector_late_interaction"
        and {"image", "document", "video"}.issubset(set(space.get("content_types") or []))
        for space in active_spaces
    ):
        raise SystemExit(
            "[error] active vector-space diagnostics did not reflect the shared ColQwen execution surface: "
            + json.dumps(diagnostics, ensure_ascii=False)
        )

    summary = {
        "status": "ok",
        "library_id": library_id,
        "job_id": job_id,
        "runtime_health_status": {
            "app": runtime_health["app"]["status"],
            "qdrant": runtime_health["qdrant"]["status"],
            "local_sidecar": local_sidecar["status"],
        },
        "execution_input_types": execution_input_types,
        "vector_space_ids": sorted(space["vector_space_id"] for space in active_spaces),
        "content_types": sorted(active_spaces[0].get("content_types") or []),
    }
    if args.json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
