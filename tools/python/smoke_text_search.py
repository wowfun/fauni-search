#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import time
import urllib.request
import argparse
from pathlib import Path
from urllib.error import HTTPError, URLError

from PIL import Image, ImageDraw


ROOT = Path.cwd()
APP_URL = f"http://{os.environ['APP_HOST']}:{os.environ['APP_PORT']}"
SIDECAR_URL = f"http://{os.environ['SIDECAR_HOST']}:{os.environ['SIDECAR_PORT']}"
QDRANT_URL = os.environ["QDRANT_URL"].rstrip("/")
APP_RUNTIME_DIR = Path(os.environ.get("APP_RUNTIME_DIR", ROOT / "data/runtime/app"))
JOB_POLL_INTERVAL_SECONDS = 1.0
JOB_POLL_TIMEOUT_SECONDS = 300.0


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


def create_pdf(path: Path) -> None:
    first_page = Image.new("RGB", (512, 512), "white")
    first_draw = ImageDraw.Draw(first_page)
    first_draw.rectangle((48, 48, 464, 464), outline="black", width=4)
    first_draw.text((80, 220), "Revenue 46 percent", fill="black")

    second_page = Image.new("RGB", (512, 512), "white")
    second_draw = ImageDraw.Draw(second_page)
    second_draw.rectangle((48, 48, 464, 464), outline="black", width=4)
    second_draw.text((80, 220), "Operating margin 18 percent", fill="black")

    first_page.save(path, "PDF", save_all=True, append_images=[second_page])


def first_fixture_image() -> tuple[Path, str]:
    manifest_path = ROOT / "tests/fixtures/tatdqa-page-images/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    entry = manifest["entries"][0]
    return (ROOT / "tests/fixtures/tatdqa-page-images" / entry["image_relpath"]).resolve(), entry["query"]


def smoke_pdf_path() -> Path:
    target_dir = APP_RUNTIME_DIR / "smoke-text-search"
    target_dir.mkdir(parents=True, exist_ok=True)
    return target_dir / "sample.pdf"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print compact machine-readable JSON")
    args = parser.parse_args()

    get_json(f"{APP_URL}/health")
    capabilities = get_json(f"{SIDECAR_URL}/capabilities")
    get_json(f"{QDRANT_URL}/collections")

    operations = {item["operation_kind"] for item in capabilities.get("operations", [])}
    if {"query_embedding", "document_embedding"} - operations:
        raise SystemExit(f"[error] sidecar capabilities are missing required operations: {operations}")

    fixture_image, query = first_fixture_image()

    pdf_path = smoke_pdf_path()
    create_pdf(pdf_path)

    create_status, created = post_json(
        f"{APP_URL}/libraries",
        {
            "display_name": "smoke-text-search",
        },
    )
    library = assert_success(create_status, created, "create library")
    library_id = library["id"]

    import_status, imported_payload = post_json(
        f"{APP_URL}/libraries/{library_id}/imports",
        {"paths": [str(fixture_image), str(pdf_path)]},
    )
    imported = assert_success(import_status, imported_payload, "import paths")
    queued_job = imported.get("job") or {}
    job_id = queued_job.get("job_id")
    if len(imported.get("accepted", [])) != 2 or not job_id:
        raise SystemExit(
            "[error] import did not return the expected queued job handle: "
            + json.dumps(imported, ensure_ascii=False)
        )
    pdf_import = next(
        (item for item in imported["accepted"] if item.get("source_type") == "pdf"),
        None,
    )
    if not pdf_import or len(pdf_import.get("assets", [])) != 2:
        raise SystemExit(
            "[error] PDF import did not expand into two document_page assets: "
            + json.dumps(imported, ensure_ascii=False)
        )
    job = wait_for_job_terminal(job_id)
    if job.get("status") != "completed" or job.get("phase") != "activated":
        raise SystemExit(
            "[error] import did not activate the multivector index: "
            + json.dumps(job, ensure_ascii=False)
        )

    search_status, searched_payload = post_json(
        f"{APP_URL}/search/text",
        {
            "library_id": library_id,
            "text": query,
            "top_k": 10,
            "debug": True,
        },
    )
    searched = assert_success(search_status, searched_payload, "text search")
    result_asset_types = {item["asset_type"] for item in searched.get("results", [])}
    debug = searched.get("debug") or {}
    if "image" not in result_asset_types or "document_page" not in result_asset_types:
        raise SystemExit(
            "[error] search did not return both image and document_page results: "
            + json.dumps(searched, ensure_ascii=False)
        )
    if (
        debug.get("backend") != "qdrant"
        or debug.get("vector_type") != "multi_vector_late_interaction"
    ):
        raise SystemExit(
            "[error] search did not report the qdrant multi_vector_late_interaction backend"
        )
    if not debug.get("vector_spaces"):
        raise SystemExit(
            "[error] search debug did not report vector_spaces: "
            + json.dumps(searched, ensure_ascii=False)
        )

    summary = {
        "status": "ok",
        "library_id": library_id,
        "job_id": job_id,
        "accepted": len(imported["accepted"]),
        "result_asset_types": sorted(result_asset_types),
        "backend": debug.get("backend"),
        "vector_type": debug.get("vector_type"),
        "vector_spaces": debug.get("vector_spaces"),
        "query_vector_count": debug.get("query_vector_count"),
        "retrieved_points": debug.get("retrieved_points"),
        "pdf_path": str(pdf_path),
    }
    if args.json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
