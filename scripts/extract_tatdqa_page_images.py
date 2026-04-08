#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import io
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Tuple

import pyarrow.parquet as pq
from PIL import Image


DEFAULT_INPUT_DIR = Path("data/datasets/tatdqa_train/data")
DEFAULT_OUTPUT_DIR = Path("tests/fixtures/tatdqa-page-images")


@dataclass
class PageImageMeta:
    split: str
    source_pdf: str
    page: str
    source_parquet: str
    query: str
    answer: str
    answer_type: str
    question_count: int
    sha256: str
    byte_size: int
    width: int
    height: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Extract unique rendered TATDQA page images from parquet shards into a "
            "small deterministic fixture set or a full export."
        )
    )
    parser.add_argument(
        "--input-dir",
        type=Path,
        default=DEFAULT_INPUT_DIR,
        help=f"Parquet shard directory. Default: {DEFAULT_INPUT_DIR}",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help=f"Output directory for manifest and images. Default: {DEFAULT_OUTPUT_DIR}",
    )
    parser.add_argument(
        "--mode",
        choices=("sample", "all"),
        default="sample",
        help="Whether to export a deterministic sample subset or all unique page images.",
    )
    parser.add_argument(
        "--sample-size",
        type=int,
        default=32,
        help="Number of unique page images to export in sample mode.",
    )
    return parser.parse_args()


def parquet_files(input_dir: Path) -> List[Path]:
    files = sorted(input_dir.glob("*.parquet"))
    if not files:
        raise SystemExit(f"No parquet files found under {input_dir}")
    return files


def split_for_file(path: Path) -> str:
    return "eval" if path.name.startswith("eval-") else "train"


def page_sort_value(page: str) -> Tuple[int, str]:
    try:
        return (0, f"{int(page):08d}")
    except ValueError:
        return (1, page)


def unique_page_metadata(files: Iterable[Path]) -> Dict[Tuple[str, str], PageImageMeta]:
    metadata: Dict[Tuple[str, str], PageImageMeta] = {}

    for path in files:
        split = split_for_file(path)
        table = pq.read_table(
            path,
            columns=["image_filename", "page", "image", "query", "answer", "answer_type"],
        )
        rows = table.to_pydict()

        for source_pdf, page, image, query, answer, answer_type in zip(
            rows["image_filename"],
            rows["page"],
            rows["image"],
            rows["query"],
            rows["answer"],
            rows["answer_type"],
        ):
            key = (source_pdf, page)
            entry = metadata.get(key)
            if entry is not None:
                entry.question_count += 1
                continue

            raw = image["bytes"]
            sha256 = hashlib.sha256(raw).hexdigest()
            with Image.open(io.BytesIO(raw)) as rendered:
                width, height = rendered.size

            metadata[key] = PageImageMeta(
                split=split,
                source_pdf=source_pdf,
                page=page,
                source_parquet=path.name,
                query=query,
                answer=answer,
                answer_type=answer_type,
                question_count=1,
                sha256=sha256,
                byte_size=len(raw),
                width=width,
                height=height,
            )

    return metadata


def selected_entries(
    metadata: Dict[Tuple[str, str], PageImageMeta], mode: str, sample_size: int
) -> List[Tuple[Tuple[str, str], PageImageMeta]]:
    ordered = sorted(
        metadata.items(),
        key=lambda item: (
            item[1].sha256,
            item[1].split,
            item[1].source_pdf,
            page_sort_value(item[1].page),
        ),
    )

    if mode == "all":
        return ordered

    by_split = {"train": [], "eval": []}
    for item in ordered:
        by_split.setdefault(item[1].split, []).append(item)

    eval_target = min(len(by_split.get("eval", [])), max(1, sample_size // 4))
    train_target = min(len(by_split.get("train", [])), sample_size - eval_target)

    chosen: List[Tuple[Tuple[str, str], PageImageMeta]] = []
    chosen.extend(by_split.get("train", [])[:train_target])
    chosen.extend(by_split.get("eval", [])[:eval_target])

    if len(chosen) < sample_size:
        chosen_keys = {item[0] for item in chosen}
        for item in ordered:
            if item[0] in chosen_keys:
                continue
            chosen.append(item)
            chosen_keys.add(item[0])
            if len(chosen) == sample_size:
                break

    return chosen


def prepare_output_dir(output_dir: Path) -> None:
    images_dir = output_dir / "images"
    images_dir.mkdir(parents=True, exist_ok=True)

    for png_path in images_dir.glob("*.png"):
        png_path.unlink()


def extract_selected_images(
    files: Iterable[Path],
    output_dir: Path,
    selected: List[Tuple[Tuple[str, str], PageImageMeta]],
) -> List[dict]:
    images_dir = output_dir / "images"

    selected_map = {
        key: {
            "id": f"tatdqa-page-{index:04d}",
            "meta": meta,
            "image_relpath": f"images/tatdqa-page-{index:04d}.png",
            "written": False,
        }
        for index, (key, meta) in enumerate(selected, start=1)
    }

    for path in files:
        table = pq.read_table(path, columns=["image_filename", "page", "image"])
        rows = table.to_pydict()

        for source_pdf, page, image in zip(
            rows["image_filename"], rows["page"], rows["image"]
        ):
            key = (source_pdf, page)
            entry = selected_map.get(key)
            if entry is None or entry["written"]:
                continue

            raw = image["bytes"]
            output_path = output_dir / entry["image_relpath"]
            output_path.write_bytes(raw)
            entry["written"] = True

    missing = [
        value["image_relpath"] for value in selected_map.values() if not value["written"]
    ]
    if missing:
        raise SystemExit(f"Failed to extract {len(missing)} selected images: {missing[:3]}")

    manifest_entries = []
    for value in selected_map.values():
        meta: PageImageMeta = value["meta"]
        manifest_entries.append(
            {
                "id": value["id"],
                "split": meta.split,
                "source_pdf": meta.source_pdf,
                "page": meta.page,
                "source_parquet": meta.source_parquet,
                "question_count": meta.question_count,
                "query": meta.query,
                "answer": meta.answer,
                "answer_type": meta.answer_type,
                "image_sha256": meta.sha256,
                "byte_size": meta.byte_size,
                "width": meta.width,
                "height": meta.height,
                "image_relpath": value["image_relpath"],
            }
        )

    return manifest_entries


def write_output_readme(
    output_dir: Path,
    input_dir: Path,
    mode: str,
    sample_size: int,
    exported_count: int,
    unique_count: int,
) -> None:
    readme = output_dir / "README.md"
    readme.write_text(
        "\n".join(
            [
                "# TATDQA Page Image Fixtures",
                "",
                "该目录由 `scripts/extract_tatdqa_page_images.py` 生成。",
                "",
                f"- 源 parquet 目录：`{input_dir}`",
                f"- 导出模式：`{mode}`",
                f"- 导出图片数：`{exported_count}` / 唯一页图总数 `{unique_count}`",
                (
                    f"- 采样大小：`{sample_size}`"
                    if mode == "sample"
                    else "- 采样大小：不适用（已导出全部唯一页图）"
                ),
                "- `manifest.json` 保存每张图片的来源页、代表性问答、尺寸与哈希。",
                "- `images/` 中的 PNG 文件按稳定顺序命名，便于后续自动化测试直接引用。",
                "",
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def write_manifest(
    output_dir: Path,
    input_dir: Path,
    mode: str,
    sample_size: int,
    unique_count: int,
    manifest_entries: List[dict],
) -> None:
    manifest = {
        "dataset": "tatdqa_train",
        "source_data_dir": input_dir.as_posix(),
        "selection_mode": mode,
        "sample_size": sample_size if mode == "sample" else None,
        "unique_page_image_count": unique_count,
        "exported_count": len(manifest_entries),
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "entries": manifest_entries,
    }
    (output_dir / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    args = parse_args()
    files = parquet_files(args.input_dir)
    metadata = unique_page_metadata(files)
    selected = selected_entries(metadata, args.mode, args.sample_size)

    prepare_output_dir(args.output_dir)
    manifest_entries = extract_selected_images(files, args.output_dir, selected)
    write_manifest(
        args.output_dir,
        args.input_dir,
        args.mode,
        args.sample_size,
        len(metadata),
        manifest_entries,
    )
    write_output_readme(
        args.output_dir,
        args.input_dir,
        args.mode,
        args.sample_size,
        len(manifest_entries),
        len(metadata),
    )

    print(
        f"Extracted {len(manifest_entries)} page images "
        f"({args.mode} mode; {len(metadata)} unique images available) "
        f"to {args.output_dir}"
    )


if __name__ == "__main__":
    main()
