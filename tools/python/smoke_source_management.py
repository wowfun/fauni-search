#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import time
import urllib.parse
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
WATCHER_POLL_TIMEOUT_SECONDS = 90.0


def get_json(url: str, timeout: int = 30) -> dict:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except URLError as exc:
        raise SystemExit(f"[error] GET {url} failed: {exc}") from exc


def request_json(method: str, url: str, payload: dict | None = None, timeout: int = 600) -> tuple[int, dict]:
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json"} if payload is not None else {}
    request = urllib.request.Request(url, data=body, headers=headers, method=method)
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
        raise SystemExit(f"[error] {method} {url} failed: {exc}") from exc


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


def list_jobs(library_id: str) -> list[dict]:
    query = urllib.parse.urlencode({"library_id": library_id})
    data = get_json(f"{APP_URL}/jobs?{query}")["data"]
    return data.get("jobs", [])


def wait_for_new_job(library_id: str, known_job_ids: set[str], kind: str | None = None) -> dict:
    deadline = time.monotonic() + WATCHER_POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        for job in list_jobs(library_id):
            if job["job_id"] in known_job_ids:
                continue
            if kind is not None and job["kind"] != kind:
                continue
            return job
        time.sleep(JOB_POLL_INTERVAL_SECONDS)
    raise SystemExit("[error] no new watcher-driven source-management job was observed in time")


def first_fixture_entries() -> tuple[dict, dict]:
    manifest_path = ROOT / "tests/fixtures/tatdqa-page-images/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    entries = manifest["entries"]
    if len(entries) < 2:
        raise SystemExit("[error] source-management smoke requires at least two committed fixture images")
    return entries[0], entries[1]


def create_pdf(path: Path, page_count: int) -> None:
    pages = []
    lines_by_page = [
        ["Q2 2025 Financial Report", "Revenue 46 percent", "Cash flow positive"],
        ["Q2 2025 Financial Report", "Operating margin 18 percent", "Forward guidance unchanged"],
    ]

    for page_index in range(page_count):
        page = Image.new("RGB", (960, 720), "white")
        draw = ImageDraw.Draw(page)
        draw.rectangle((60, 60, 900, 660), outline="black", width=6)
        for line_index, line in enumerate(lines_by_page[page_index % len(lines_by_page)]):
            draw.text((120, 170 + line_index * 90), line, fill="black")
        pages.append(page)

    pages[0].save(path, "PDF", save_all=True, append_images=pages[1:])


def create_source_management_fixtures(target_dir: Path) -> dict:
    if target_dir.exists():
        shutil.rmtree(target_dir)
    target_dir.mkdir(parents=True, exist_ok=True)
    first_entry, second_entry = first_fixture_entries()
    first_image = (ROOT / "tests/fixtures/tatdqa-page-images" / first_entry["image_relpath"]).resolve()
    second_image = (ROOT / "tests/fixtures/tatdqa-page-images" / second_entry["image_relpath"]).resolve()

    managed_image_path = target_dir / "chart.png"
    added_image_path = target_dir / "new-chart.png"
    pdf_path = target_dir / "report.pdf"
    shutil.copy2(first_image, managed_image_path)
    create_pdf(pdf_path, 2)

    return {
        "managed_image_path": managed_image_path,
        "added_image_source_path": second_image,
        "added_image_path": added_image_path,
        "pdf_path": pdf_path,
        "image_query": first_entry["query"],
        "pdf_query": "Revenue 46 percent",
    }


def list_sources(library_id: str, **filters: str) -> list[dict]:
    query = urllib.parse.urlencode({key: value for key, value in filters.items() if value})
    url = f"{APP_URL}/libraries/{library_id}/sources"
    if query:
        url = f"{url}?{query}"
    data = get_json(url)["data"]
    return data.get("sources", [])


def source_by_path(sources: list[dict], suffix: str) -> dict:
    for source in sources:
        if source["source_path"].endswith(suffix):
            return source
    raise SystemExit(f"[error] source inventory did not contain a source ending with {suffix}")


def run_text_search(library_id: str, text: str) -> dict:
    status, payload = request_json(
        "POST",
        f"{APP_URL}/search/text",
        {
            "library_id": library_id,
            "text": text,
            "top_k": 10,
            "debug": True,
        },
    )
    return assert_success(status, payload, f"text search for {text!r}")


def assert_contains_source(label: str, search_data: dict, source_id: str) -> None:
    source_ids = {item["source_id"] for item in search_data.get("results", [])}
    if source_id not in source_ids:
        raise SystemExit(
            f"[error] {label} did not contain expected source {source_id}: "
            + json.dumps(search_data, ensure_ascii=False)
        )


def assert_excludes_source(label: str, search_data: dict, source_id: str) -> None:
    source_ids = {item["source_id"] for item in search_data.get("results", [])}
    if source_id in source_ids:
        raise SystemExit(
            f"[error] {label} still contained invalidated/out_of_scope source {source_id}: "
            + json.dumps(search_data, ensure_ascii=False)
        )


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

    target_dir = APP_RUNTIME_DIR / "smoke-source-management"
    fixtures = create_source_management_fixtures(target_dir)

    create_status, created = request_json(
        "POST",
        f"{APP_URL}/libraries",
        {
            "name": "smoke-source-management",
            "config": {"enabled_index_lines": ["multivector"]},
        },
    )
    library = assert_success(create_status, created, "create library")
    library_id = library["id"]

    root_status, created_root_payload = request_json(
        "POST",
        f"{APP_URL}/libraries/{library_id}/source-roots",
        {
            "root_path": str(target_dir),
            "enabled": True,
            "rules": {},
        },
    )
    source_root = assert_success(root_status, created_root_payload, "create source root")
    source_root_id = source_root["source_root_id"]

    refresh_status, refresh_payload = request_json(
        "POST",
        f"{APP_URL}/libraries/{library_id}/source-roots/{source_root_id}/refresh",
    )
    refresh_receipt = assert_success(refresh_status, refresh_payload, "initial source-root refresh")
    refresh_job = refresh_receipt.get("job") or {}
    refresh_job_id = refresh_job.get("job_id")
    if not refresh_job_id:
        raise SystemExit("[error] initial source-root refresh did not return a job handle")
    refresh_job = wait_for_job_terminal(refresh_job_id)
    if refresh_job.get("status") != "completed":
        raise SystemExit(
            "[error] initial source-root refresh did not complete successfully: "
            + json.dumps(refresh_job, ensure_ascii=False)
        )

    known_job_ids = {job["job_id"] for job in list_jobs(library_id)}

    active_sources = list_sources(library_id, status="active")
    if len(active_sources) != 2:
        raise SystemExit(
            "[error] initial source inventory did not contain two active managed sources: "
            + json.dumps(active_sources, ensure_ascii=False)
        )

    image_source = source_by_path(active_sources, "chart.png")
    pdf_source = source_by_path(active_sources, "report.pdf")
    if pdf_source.get("visual_unit_count") != 2:
        raise SystemExit(
            "[error] initial managed PDF did not expose two active document_page visual units: "
            + json.dumps(pdf_source, ensure_ascii=False)
        )

    image_search = run_text_search(library_id, fixtures["image_query"])
    assert_contains_source("initial managed-image search", image_search, image_source["source_id"])

    pdf_search = run_text_search(library_id, fixtures["pdf_query"])
    assert_contains_source("initial managed-pdf search", pdf_search, pdf_source["source_id"])

    shutil.copy2(fixtures["added_image_source_path"], fixtures["added_image_path"])
    watcher_add_job = wait_for_new_job(library_id, known_job_ids, kind="refresh")
    known_job_ids.add(watcher_add_job["job_id"])
    watcher_add_job = wait_for_job_terminal(watcher_add_job["job_id"])
    if watcher_add_job.get("status") != "completed":
        raise SystemExit("[error] watcher add job did not complete successfully")

    active_sources = list_sources(library_id, status="active")
    if len(active_sources) != 3:
        raise SystemExit(
            "[error] watcher-driven add did not produce a third active source: "
            + json.dumps(active_sources, ensure_ascii=False)
        )
    added_image_source = source_by_path(active_sources, "new-chart.png")

    create_pdf(fixtures["pdf_path"], 1)
    watcher_modify_job = wait_for_new_job(library_id, known_job_ids, kind="refresh")
    known_job_ids.add(watcher_modify_job["job_id"])
    watcher_modify_job = wait_for_job_terminal(watcher_modify_job["job_id"])
    if watcher_modify_job.get("status") != "completed":
        raise SystemExit("[error] watcher modify job did not complete successfully")

    active_sources = list_sources(library_id, status="active")
    modified_pdf_source = source_by_path(active_sources, "report.pdf")
    if modified_pdf_source.get("visual_unit_count") != 1:
        raise SystemExit(
            "[error] watcher-driven modify did not shrink the managed PDF to one visual unit: "
            + json.dumps(modified_pdf_source, ensure_ascii=False)
        )

    fixtures["managed_image_path"].unlink()
    watcher_delete_job = wait_for_new_job(library_id, known_job_ids, kind="refresh")
    known_job_ids.add(watcher_delete_job["job_id"])
    watcher_delete_job = wait_for_job_terminal(watcher_delete_job["job_id"])
    if watcher_delete_job.get("status") != "completed":
        raise SystemExit("[error] watcher delete job did not complete successfully")

    invalidated_sources = list_sources(library_id, status="invalidated")
    invalidated_image_source = source_by_path(invalidated_sources, "chart.png")
    if invalidated_image_source.get("status_reason") != "not_found":
        raise SystemExit(
            "[error] deleted managed image did not enter invalidated/not_found state: "
            + json.dumps(invalidated_image_source, ensure_ascii=False)
        )

    deleted_image_search = run_text_search(library_id, fixtures["image_query"])
    assert_excludes_source(
        "post-delete managed-image search",
        deleted_image_search,
        image_source["source_id"],
    )

    patch_status, patch_payload = request_json(
        "PATCH",
        f"{APP_URL}/libraries/{library_id}/source-roots/{source_root_id}",
        {
            "rules": {
                "exclude_globs": ["report.pdf"],
            }
        },
    )
    assert_success(patch_status, patch_payload, "patch source-root rules")

    library_refresh_status, library_refresh_payload = request_json(
        "POST",
        f"{APP_URL}/libraries/{library_id}/refresh",
    )
    library_refresh = assert_success(
        library_refresh_status,
        library_refresh_payload,
        "library refresh after source-root rule update",
    )
    library_refresh_job = library_refresh.get("job") or {}
    library_refresh_job_id = library_refresh_job.get("job_id")
    if not library_refresh_job_id:
        raise SystemExit("[error] library refresh after rule update did not return a job handle")
    known_job_ids.add(library_refresh_job_id)
    library_refresh_job = wait_for_job_terminal(library_refresh_job_id)
    if library_refresh_job.get("status") != "completed":
        raise SystemExit("[error] library refresh after rule update did not complete successfully")

    out_of_scope_sources = list_sources(library_id, status="out_of_scope")
    out_of_scope_pdf_source = source_by_path(out_of_scope_sources, "report.pdf")
    if out_of_scope_pdf_source.get("status_reason") != "rule_excluded":
        raise SystemExit(
            "[error] rule-updated managed PDF did not enter out_of_scope/rule_excluded state: "
            + json.dumps(out_of_scope_pdf_source, ensure_ascii=False)
        )

    pdf_search_after_rule_update = run_text_search(library_id, fixtures["pdf_query"])
    assert_excludes_source(
        "post-rule-update managed-pdf search",
        pdf_search_after_rule_update,
        pdf_source["source_id"],
    )

    summary = {
        "status": "ok",
        "library_id": library_id,
        "source_root_id": source_root_id,
        "initial_refresh_job_id": refresh_job_id,
        "watcher_add_job_id": watcher_add_job["job_id"],
        "watcher_modify_job_id": watcher_modify_job["job_id"],
        "watcher_delete_job_id": watcher_delete_job["job_id"],
        "rule_refresh_job_id": library_refresh_job_id,
        "active_source_ids_after_add": sorted(
            [image_source["source_id"], pdf_source["source_id"], added_image_source["source_id"]]
        ),
        "invalidated_source_id": invalidated_image_source["source_id"],
        "out_of_scope_source_id": out_of_scope_pdf_source["source_id"],
        "managed_root_path": str(target_dir),
    }
    if args.json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
