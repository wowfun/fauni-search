#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path


ROOT = Path.cwd()
DEFAULT_RUNTIME_DIR = ROOT / "data" / "runtime" / "video-artifacts"


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"[error] {message}")


def require_tool(name: str) -> None:
    if shutil.which(name) is None:
        fail(f"{name} is required but was not found in PATH")


def load_manifest(path: Path) -> dict:
    if not path.is_file():
        fail(f"manifest not found: {path}")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        fail(f"manifest is not valid JSON: {exc}")


def format_ms(ms: int) -> str:
    total_seconds = ms / 1000.0
    hours = int(total_seconds // 3600)
    minutes = int((total_seconds % 3600) // 60)
    seconds = total_seconds % 60
    return f"{hours:02d}:{minutes:02d}:{seconds:06.3f}"


def ensure_moment(moment: dict, duration_ms: int) -> dict:
    for key in ("id", "label", "start_ms", "end_ms"):
        if key not in moment:
            fail(f"moment is missing required field '{key}': {moment}")
    try:
        start_ms = int(moment["start_ms"])
        end_ms = int(moment["end_ms"])
    except (TypeError, ValueError):
        fail(f"moment has non-integer start_ms/end_ms: {moment}")
    if start_ms < 0 or end_ms <= start_ms:
        fail(f"moment has invalid range {start_ms}-{end_ms}: {moment['id']}")
    if duration_ms and end_ms > duration_ms:
        fail(f"moment range {start_ms}-{end_ms} exceeds duration {duration_ms}: {moment['id']}")
    return {
        **moment,
        "start_ms": start_ms,
        "end_ms": end_ms,
        "mid_ms": start_ms + ((end_ms - start_ms) // 2),
    }


def build_output_dir(manifest_path: Path, manifest: dict, output_dir: Path | None) -> Path:
    if output_dir is not None:
        return output_dir
    source_name = Path(manifest.get("source_name", manifest_path.stem)).stem
    return DEFAULT_RUNTIME_DIR / source_name


def extract_frame(video_path: Path, output_path: Path, timestamp_ms: int) -> None:
    command = [
        "ffmpeg",
        "-y",
        "-ss",
        format_ms(timestamp_ms),
        "-i",
        str(video_path),
        "-frames:v",
        "1",
        "-q:v",
        "2",
        str(output_path),
    ]
    run_subprocess(command, f"extract frame {output_path.name}")


def extract_clip(video_path: Path, output_path: Path, start_ms: int, end_ms: int) -> None:
    duration_ms = end_ms - start_ms
    command = [
        "ffmpeg",
        "-y",
        "-ss",
        format_ms(start_ms),
        "-i",
        str(video_path),
        "-t",
        format_ms(duration_ms),
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "23",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        str(output_path),
    ]
    run_subprocess(command, f"extract clip {output_path.name}")


def run_subprocess(command: list[str], label: str) -> None:
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        fail(f"{label} failed: {stderr or 'unknown ffmpeg error'}")


def derive_artifacts(
    video_path: Path,
    manifest_path: Path,
    manifest: dict,
    output_dir: Path,
    emit_frames: bool,
    emit_clips: bool,
) -> dict:
    duration_ms = int(manifest.get("duration_ms") or 0)
    moments = [ensure_moment(moment, duration_ms) for moment in manifest.get("moments", [])]
    if not moments:
        fail("manifest does not contain any moments")

    frames_dir = output_dir / "frames"
    clips_dir = output_dir / "clips"
    output_dir.mkdir(parents=True, exist_ok=True)
    if emit_frames:
        frames_dir.mkdir(parents=True, exist_ok=True)
    if emit_clips:
        clips_dir.mkdir(parents=True, exist_ok=True)

    derived_moments: list[dict] = []
    for moment in moments:
        frame_path = frames_dir / f"{moment['id']}.png"
        clip_path = clips_dir / f"{moment['id']}.mp4"
        if emit_frames:
            extract_frame(video_path, frame_path, moment["mid_ms"])
        if emit_clips:
            extract_clip(video_path, clip_path, moment["start_ms"], moment["end_ms"])
        derived_moments.append(
            {
                "id": moment["id"],
                "label": moment["label"],
                "start_ms": moment["start_ms"],
                "end_ms": moment["end_ms"],
                "mid_ms": moment["mid_ms"],
                "frame_path": str(frame_path.relative_to(ROOT)) if emit_frames else None,
                "clip_path": str(clip_path.relative_to(ROOT)) if emit_clips else None,
                "expected_result_kinds": moment.get("expected_result_kinds", []),
                "keywords": moment.get("keywords", []),
            }
        )

    index_payload = {
        "fixture_kind": "derived_video_artifacts",
        "source_manifest": str(manifest_path.relative_to(ROOT)),
        "source_video": str(video_path.relative_to(ROOT)),
        "output_dir": str(output_dir.relative_to(ROOT)),
        "frames_enabled": emit_frames,
        "clips_enabled": emit_clips,
        "moments": derived_moments,
    }
    index_path = output_dir / "index.json"
    index_path.write_text(json.dumps(index_payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return index_payload


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Extract reusable screenshots and clips from a local video manifest")
    parser.add_argument("--manifest", required=True, type=Path, help="path to the local-only video manifest JSON")
    parser.add_argument("--video", type=Path, help="override source video path; defaults to the manifest source path")
    parser.add_argument("--output-dir", type=Path, help="override artifact output directory")
    parser.add_argument("--frames", action="store_true", help="extract frame PNGs only")
    parser.add_argument("--clips", action="store_true", help="extract clip MP4s only")
    parser.add_argument("--all", action="store_true", help="extract both frames and clips")
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON summary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not (args.frames or args.clips or args.all):
        args.all = True
    emit_frames = args.all or args.frames
    emit_clips = args.all or args.clips

    require_tool("ffmpeg")
    require_tool("ffprobe")

    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    video_path = (args.video or ROOT / manifest.get("source" "_path", "")).resolve()
    if not video_path.is_file():
        fail(f"video not found: {video_path}")

    output_dir = build_output_dir(manifest_path, manifest, args.output_dir.resolve() if args.output_dir else None)
    result = derive_artifacts(video_path, manifest_path, manifest, output_dir, emit_frames, emit_clips)

    if args.json:
        print(json.dumps(result, ensure_ascii=False))
    else:
        print(f"source_video: {result['source_video']}")
        print(f"output_dir: {result['output_dir']}")
        print(f"frames_enabled: {result['frames_enabled']}")
        print(f"clips_enabled: {result['clips_enabled']}")
        print(f"moment_count: {len(result['moments'])}")
        for moment in result["moments"]:
            frame_path = moment["frame_path"] or "-"
            clip_path = moment["clip_path"] or "-"
            print(
                f"- {moment['id']}: {moment['start_ms']}-{moment['end_ms']} "
                f"frame={frame_path} clip={clip_path}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
