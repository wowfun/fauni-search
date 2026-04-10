#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import time
import urllib.request
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


def post_multipart(url: str, field_name: str, file_path: Path, content_type: str, timeout: int = 120) -> tuple[int, dict]:
    boundary = f"fauni-search-{int(time.time() * 1000)}"
    body = bytearray()
    body.extend(f"--{boundary}\r\n".encode("utf-8"))
    body.extend(
        (
            f'Content-Disposition: form-data; name="{field_name}"; filename="{file_path.name}"\r\n'
            f"Content-Type: {content_type}\r\n\r\n"
        ).encode("utf-8")
    )
    body.extend(file_path.read_bytes())
    body.extend(f"\r\n--{boundary}--\r\n".encode("utf-8"))

    request = urllib.request.Request(
        url,
        data=bytes(body),
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except HTTPError as exc:
        body_text = exc.read().decode("utf-8")
        try:
            parsed = json.loads(body_text)
        except json.JSONDecodeError:
            parsed = {"raw": body_text}
        return exc.code, parsed
    except URLError as exc:
        raise SystemExit(f"[error] multipart POST {url} failed: {exc}") from exc


def assert_success(status: int, payload: dict, label: str) -> dict:
    if status < 200 or status >= 300:
        raise SystemExit(
            f"[error] {label} failed with HTTP {status}: {json.dumps(payload, ensure_ascii=False)}"
        )
    if "data" not in payload:
        raise SystemExit(
            f"[error] {label} did not return a data envelope: {json.dumps(payload, ensure_ascii=False)}"
        )
    return payload["data"]


def wait_for_job_terminal(job_id: str) -> dict:
    deadline = time.monotonic() + JOB_POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        job = get_json(f"{APP_URL}/jobs/{job_id}")["data"]
        if job["status"] in {"completed", "failed", "canceled"}:
            return job
        time.sleep(JOB_POLL_INTERVAL_SECONDS)
    raise SystemExit(f"[error] job {job_id} did not reach a terminal state in time")


def create_document_search_fixtures(target_dir: Path) -> tuple[Path, Path]:
    target_dir.mkdir(parents=True, exist_ok=True)
    image_path = target_dir / "report-page.png"
    pdf_path = target_dir / "query-document.pdf"

    first_page = Image.new("RGB", (960, 720), "white")
    first_draw = ImageDraw.Draw(first_page)
    first_draw.rectangle((60, 60, 900, 660), outline="black", width=6)
    first_draw.text((120, 170), "Q2 2025 Financial Report", fill="black")
    first_draw.text((120, 260), "Revenue 46 percent", fill="black")
    first_draw.text((120, 350), "Net income 18 percent", fill="black")
    first_draw.text((120, 440), "Cash flow positive", fill="black")

    second_page = Image.new("RGB", (960, 720), "white")
    second_draw = ImageDraw.Draw(second_page)
    second_draw.rectangle((60, 60, 900, 660), outline="black", width=6)
    second_draw.text((120, 170), "Q2 2025 Financial Report", fill="black")
    second_draw.text((120, 260), "Operating margin 18 percent", fill="black")
    second_draw.text((120, 350), "Cash conversion stable", fill="black")
    second_draw.text((120, 440), "Forward guidance unchanged", fill="black")

    first_page.save(image_path, "PNG")
    first_page.save(pdf_path, "PDF", save_all=True, append_images=[second_page])
    return image_path, pdf_path


def assert_result_kinds(label: str, search_data: dict, required_kinds: set[str]) -> set[str]:
    result_kinds = {item["kind"] for item in search_data.get("results", [])}
    if missing := required_kinds - result_kinds:
        raise SystemExit(
            f"[error] {label} did not return required result kinds {sorted(missing)}: "
            + json.dumps(search_data, ensure_ascii=False)
        )
    debug = search_data.get("debug") or {}
    if debug.get("backend") != "qdrant" or debug.get("repr_kind") != "multivector":
        raise SystemExit(f"[error] {label} did not report the qdrant multivector backend")
    return result_kinds


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true", help="print compact machine-readable JSON")
    args = parser.parse_args()

    get_json(f"{APP_URL}/health")
    capabilities = get_json(f"{SIDECAR_URL}/capabilities")
    get_json(f"{QDRANT_URL}/collections")

    operations = {item["operation_kind"] for item in capabilities.get("operations", [])}
    if {"document_query_embedding", "document_embedding"} - operations:
        raise SystemExit(f"[error] sidecar capabilities are missing required operations: {operations}")

    target_dir = APP_RUNTIME_DIR / "smoke-document-search"
    image_path, pdf_path = create_document_search_fixtures(target_dir)

    create_status, created = post_json(
        f"{APP_URL}/libraries",
        {
            "name": "smoke-document-search",
            "config": {"enabled_index_lines": ["multivector"]},
        },
    )
    library = assert_success(create_status, created, "create library")
    library_id = library["id"]

    import_status, imported_payload = post_json(
        f"{APP_URL}/libraries/{library_id}/imports",
        {"paths": [str(image_path), str(pdf_path)]},
    )
    imported = assert_success(import_status, imported_payload, "import paths")
    queued_job = imported.get("job") or {}
    job_id = queued_job.get("job_id")
    if len(imported.get("accepted", [])) != 2 or not job_id:
        raise SystemExit(
            "[error] import did not return the expected queued job handle: "
            + json.dumps(imported, ensure_ascii=False)
        )

    document_import = next((item for item in imported["accepted"] if item.get("source_type") == "pdf"), None)
    if not document_import or len(document_import.get("visual_units", [])) != 2:
        raise SystemExit(
            "[error] PDF import did not expand into two document_page visual units: "
            + json.dumps(imported, ensure_ascii=False)
        )

    source_id = document_import.get("source_id")
    if not source_id:
        raise SystemExit("[error] PDF import did not return source_id")

    job = wait_for_job_terminal(job_id)
    if job.get("status") != "completed" or job.get("phase") != "activated":
        raise SystemExit(
            "[error] import did not activate the multivector index: "
            + json.dumps(job, ensure_ascii=False)
        )

    upload_status, upload_payload = post_multipart(
        f"{APP_URL}/libraries/{library_id}/query-assets/documents",
        "file",
        pdf_path,
        "application/pdf",
    )
    uploaded = assert_success(upload_status, upload_payload, "upload query document")
    temp_asset_id = uploaded.get("temp_asset_id")
    if not temp_asset_id:
        raise SystemExit("[error] query document upload did not return temp_asset_id")

    temp_search_status, temp_search_payload = post_json(
        f"{APP_URL}/search/document",
        {
            "library_id": library_id,
            "document_input": {
                "kind": "temp_asset",
                "temp_asset_id": temp_asset_id,
            },
            "top_k": 10,
            "debug": True,
        },
    )
    temp_search = assert_success(temp_search_status, temp_search_payload, "temp-asset document search")
    temp_result_kinds = assert_result_kinds("temp-asset document search", temp_search, {"document_page", "image"})

    source_search_status, source_search_payload = post_json(
        f"{APP_URL}/search/document",
        {
            "library_id": library_id,
            "document_input": {
                "kind": "library_object",
                "source_id": source_id,
            },
            "top_k": 10,
            "debug": True,
        },
    )
    source_search = assert_success(source_search_status, source_search_payload, "source-id document search")
    source_result_kinds = assert_result_kinds("source-id document search", source_search, {"document_page", "image"})

    ranged_search_status, ranged_search_payload = post_json(
        f"{APP_URL}/search/document",
        {
            "library_id": library_id,
            "document_input": {
                "kind": "library_object",
                "source_id": source_id,
                "locator": {
                    "start_page": 2,
                    "end_page": 2,
                },
            },
            "top_k": 10,
            "debug": True,
        },
    )
    ranged_search = assert_success(ranged_search_status, ranged_search_payload, "ranged document search")
    ranged_result_kinds = assert_result_kinds("ranged document search", ranged_search, {"document_page", "image"})

    document_page_result = next(
        (item for item in temp_search.get("results", []) if item.get("kind") == "document_page"),
        None,
    )
    if not document_page_result:
        raise SystemExit(
            "[error] temp-asset document search did not return a document_page result that can be reused"
        )
    reuse_page = document_page_result.get("locator", {}).get("page")
    if not reuse_page:
        raise SystemExit("[error] document_page result did not expose locator.page")

    reuse_search_status, reuse_search_payload = post_json(
        f"{APP_URL}/search/document",
        {
            "library_id": library_id,
            "document_input": {
                "kind": "library_object",
                "source_id": document_page_result["source_id"],
                "locator": {
                    "start_page": reuse_page,
                    "end_page": reuse_page,
                },
            },
            "top_k": 10,
            "debug": True,
        },
    )
    reuse_search = assert_success(reuse_search_status, reuse_search_payload, "document_page reuse search")
    reuse_result_kinds = assert_result_kinds("document_page reuse search", reuse_search, {"document_page", "image"})

    summary = {
        "status": "ok",
        "library_id": library_id,
        "job_id": job_id,
        "source_id": source_id,
        "temp_asset_id": temp_asset_id,
        "result_kinds": sorted(temp_result_kinds),
        "source_id_result_kinds": sorted(source_result_kinds),
        "ranged_result_kinds": sorted(ranged_result_kinds),
        "document_page_reuse_result_kinds": sorted(reuse_result_kinds),
        "backend": temp_search.get("debug", {}).get("backend"),
        "repr_kind": temp_search.get("debug", {}).get("repr_kind"),
        "pdf_path": str(pdf_path),
        "image_path": str(image_path),
    }
    if args.json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
