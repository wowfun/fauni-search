#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import time
import urllib.request
from pathlib import Path
from urllib.error import HTTPError, URLError

from PIL import Image

from extract_video_artifacts import build_output_dir, derive_artifacts, load_manifest


ROOT = Path.cwd()
DEFAULT_MANIFEST = ROOT / "data/generate_q2_report_from_csv_bank_data-720-512.local.manifest.json"
APP_URL = f"http://{os.environ['APP_HOST']}:{os.environ['APP_PORT']}"
SIDECAR_URL = f"http://{os.environ['SIDECAR_HOST']}:{os.environ['SIDECAR_PORT']}"
QDRANT_URL = os.environ["QDRANT_URL"].rstrip("/")
APP_RUNTIME_DIR = Path(os.environ.get("APP_RUNTIME_DIR", ROOT / "data/runtime/app"))
JOB_POLL_INTERVAL_SECONDS = 1.0
JOB_POLL_TIMEOUT_SECONDS = 600.0


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


def post_multipart(
    url: str, field_name: str, file_path: Path, content_type: str, timeout: int = 600
) -> tuple[int, dict]:
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


def select_manifest(path: Path | None) -> Path:
    candidate = (path or DEFAULT_MANIFEST).resolve()
    if not candidate.is_file():
        raise SystemExit(
            f"[error] local video smoke manifest not found: {candidate}. "
            "Provide --manifest or create the local-only manifest first."
        )
    return candidate


def ensure_runtime_ready() -> None:
    get_json(f"{APP_URL}/health")
    capabilities = get_json(f"{SIDECAR_URL}/capabilities")
    get_json(f"{QDRANT_URL}/collections")

    operations = {item["operation_kind"] for item in capabilities.get("operations", [])}
    required = {"video_query_embedding", "document_embedding"}
    missing = required - operations
    if missing:
        raise SystemExit(
            f"[error] sidecar capabilities are missing required operations: {sorted(missing)}"
        )


def select_primary_moment(moments: list[dict]) -> dict:
    for moment in moments:
        expected = set(moment.get("expected_result_kinds", []))
        if {"image", "document_page"} & expected:
            return moment
    return moments[0]


def create_pdf_from_frame(frame_path: Path, output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with Image.open(frame_path) as image:
        image.convert("RGB").save(output_path, "PDF")


def smoke_pdf_path() -> Path:
    target_dir = APP_RUNTIME_DIR / "smoke-video-search"
    target_dir.mkdir(parents=True, exist_ok=True)
    return target_dir / "reference-frame.pdf"


def search_video(
    library_id: str,
    video_input: dict,
    *,
    debug: bool = True,
    top_k: int = 10,
) -> dict:
    search_status, search_payload = post_json(
        f"{APP_URL}/search/video",
        {
            "library_id": library_id,
            "video_input": video_input,
            "top_k": top_k,
            "debug": debug,
        },
    )
    return assert_success(search_status, search_payload, "video search")


def require_result_asset_types(payload: dict, expected: set[str], label: str) -> None:
    result_asset_types = {item["asset_type"] for item in payload.get("results", [])}
    if not expected.issubset(result_asset_types):
        raise SystemExit(
            f"[error] {label} did not return expected result asset types {sorted(expected)}: "
            + json.dumps(payload, ensure_ascii=False)
        )
    debug = payload.get("debug") or {}
    if (
        debug.get("backend") != "qdrant"
        or debug.get("vector_type") != "multi_vector_late_interaction"
    ):
        raise SystemExit(
            f"[error] {label} did not report the qdrant multi_vector_late_interaction backend: "
            + json.dumps(payload, ensure_ascii=False)
        )


def main() -> int:
    parser = argparse.ArgumentParser(description="Smoke test the 120-video-search runtime path")
    parser.add_argument("--manifest", type=Path, help="path to a local-only video manifest JSON")
    parser.add_argument("--video", type=Path, help="override source video path")
    parser.add_argument("--output-dir", type=Path, help="override artifact output directory")
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    args = parser.parse_args()

    manifest_path = select_manifest(args.manifest)
    manifest = load_manifest(manifest_path)
    video_path = (args.video or ROOT / manifest.get("source" "_path", "")).resolve()
    if not video_path.is_file():
        raise SystemExit(f"[error] local video smoke source is missing: {video_path}")

    output_dir = build_output_dir(
        manifest_path,
        manifest,
        args.output_dir.resolve() if args.output_dir else None,
    )
    derived = derive_artifacts(
        video_path=video_path,
        manifest_path=manifest_path,
        manifest=manifest,
        output_dir=output_dir,
        emit_frames=True,
        emit_clips=True,
    )
    primary_moment = select_primary_moment(derived["moments"])
    frame_path = (ROOT / primary_moment["frame_path"]).resolve()
    clip_path = (ROOT / primary_moment["clip_path"]).resolve()
    pdf_path = smoke_pdf_path()
    create_pdf_from_frame(frame_path, pdf_path)

    ensure_runtime_ready()

    create_status, created_payload = post_json(
        f"{APP_URL}/libraries",
        {
            "display_name": "smoke-video-search",
        },
    )
    library = assert_success(create_status, created_payload, "create library")
    library_id = library["id"]

    import_status, imported_payload = post_json(
        f"{APP_URL}/libraries/{library_id}/imports",
        {"paths": [str(video_path), str(frame_path), str(pdf_path)]},
    )
    imported = assert_success(import_status, imported_payload, "import paths")
    queued_job = imported.get("job") or {}
    job_id = queued_job.get("job_id")
    if len(imported.get("accepted", [])) != 3 or not job_id:
        raise SystemExit(
            "[error] import did not return the expected queued job handle: "
            + json.dumps(imported, ensure_ascii=False)
        )

    video_source = next(
        (item for item in imported["accepted"] if item.get("source_type") == "video"),
        None,
    )
    if not video_source or not video_source.get("source_id"):
        raise SystemExit(
            "[error] import did not expose a reusable video source_id: "
            + json.dumps(imported, ensure_ascii=False)
        )

    job = wait_for_job_terminal(job_id)
    if job.get("status") != "completed" or job.get("phase") != "activated":
        raise SystemExit(
            "[error] import did not activate the multivector index: "
            + json.dumps(job, ensure_ascii=False)
        )

    upload_status, upload_payload = post_multipart(
        f"{APP_URL}/libraries/{library_id}/query-assets/videos",
        "file",
        video_path,
        "video/mp4",
    )
    uploaded = assert_success(upload_status, upload_payload, "upload query video")
    temp_asset_id = uploaded.get("temp_asset_id")
    if not temp_asset_id:
        raise SystemExit("[error] query video upload did not return temp_asset_id")

    temp_asset_search = search_video(
        library_id,
        {
            "kind": "temp_asset",
            "temp_asset_id": temp_asset_id,
            "locator": {
                "start_ms": primary_moment["start_ms"],
                "end_ms": primary_moment["end_ms"],
            },
        },
    )
    require_result_asset_types(
        temp_asset_search,
        {"video_segment", "image", "document_page"},
        "temp-asset video search",
    )

    clip_upload_status, clip_upload_payload = post_multipart(
        f"{APP_URL}/libraries/{library_id}/query-assets/videos",
        "file",
        clip_path,
        "video/mp4",
    )
    clip_uploaded = assert_success(clip_upload_status, clip_upload_payload, "upload query clip")
    clip_temp_asset_id = clip_uploaded.get("temp_asset_id")
    if not clip_temp_asset_id:
        raise SystemExit("[error] query clip upload did not return temp_asset_id")

    whole_clip_search = search_video(
        library_id,
        {
            "kind": "temp_asset",
            "temp_asset_id": clip_temp_asset_id,
        },
    )
    require_result_asset_types(
        whole_clip_search,
        {"video_segment", "image", "document_page"},
        "whole-clip video search",
    )

    library_object_search = search_video(
        library_id,
        {
            "kind": "library_object",
            "source_id": video_source["source_id"],
            "locator": {
                "start_ms": primary_moment["start_ms"],
                "end_ms": primary_moment["end_ms"],
            },
        },
    )
    require_result_asset_types(
        library_object_search,
        {"video_segment", "image", "document_page"},
        "library-object video search",
    )

    first_video_segment = next(
        (
            item
            for item in temp_asset_search.get("results", [])
            if item.get("asset_type") == "video_segment" and item.get("asset_id")
        ),
        None,
    )
    if not first_video_segment:
        raise SystemExit(
            "[error] temp-asset video search did not return a reusable video_segment result: "
            + json.dumps(temp_asset_search, ensure_ascii=False)
        )

    video_segment_object_search = search_video(
        library_id,
        {
            "kind": "library_object",
            "asset_id": first_video_segment["asset_id"],
        },
    )
    require_result_asset_types(
        video_segment_object_search,
        {"video_segment", "image", "document_page"},
        "video-segment library-object search",
    )

    payload = {
        "status": "ok",
        "config_source": os.environ.get("FAUNI_CONFIG_SOURCE", ""),
        "manifest": str(manifest_path.relative_to(ROOT)),
        "video": str(video_path.relative_to(ROOT)),
        "artifact_output_dir": derived["output_dir"],
        "moment_id": primary_moment["id"],
        "temp_asset_duration_ms": uploaded.get("duration_ms"),
        "clip_temp_asset_duration_ms": clip_uploaded.get("duration_ms"),
        "result_asset_types": sorted({item["asset_type"] for item in temp_asset_search["results"]}),
        "whole_clip_result_asset_types": sorted({item["asset_type"] for item in whole_clip_search["results"]}),
        "library_object_result_asset_types": sorted(
            {item["asset_type"] for item in library_object_search["results"]}
        ),
        "video_segment_object_result_asset_types": sorted(
            {item["asset_type"] for item in video_segment_object_search["results"]}
        ),
        "backend": temp_asset_search.get("debug", {}).get("backend"),
        "vector_type": temp_asset_search.get("debug", {}).get("vector_type"),
    }

    if args.json:
        print(json.dumps(payload, ensure_ascii=False))
    else:
        print(f"status: {payload['status']}")
        if payload["config_source"]:
            print(f"config_source: {payload['config_source']}")
        print(f"manifest: {payload['manifest']}")
        print(f"video: {payload['video']}")
        print(f"artifact_output_dir: {payload['artifact_output_dir']}")
        print(f"moment_id: {payload['moment_id']}")
        print(f"temp_asset_duration_ms: {payload['temp_asset_duration_ms']}")
        print(f"clip_temp_asset_duration_ms: {payload['clip_temp_asset_duration_ms']}")
        print(f"result_asset_types: {payload['result_asset_types']}")
        print(f"whole_clip_result_asset_types: {payload['whole_clip_result_asset_types']}")
        print(f"library_object_result_asset_types: {payload['library_object_result_asset_types']}")
        print(
            "video_segment_object_result_asset_types: "
            f"{payload['video_segment_object_result_asset_types']}"
        )
        print(f"backend: {payload['backend']}")
        print(f"vector_type: {payload['vector_type']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
