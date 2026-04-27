from __future__ import annotations

import logging
import re
import sys
from pathlib import Path

from fauni_sidecar.modeld import configure_modeld_logging


TIMESTAMPED_LINE = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z  "
    r"( INFO| WARN|ERROR) [\w.]+: .+"
)


def test_modeld_logger_writes_utc_timestamp_level_and_logger_name(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "modeld.log"
    state = configure_modeld_logging(
        log_path,
        max_bytes=1024,
        backup_count=2,
        startup_rollover=False,
    )
    try:
        logging.getLogger("uvicorn.access").info("GET /health")
    finally:
        state.close()

    lines = log_path.read_text(encoding="utf-8").splitlines()
    assert TIMESTAMPED_LINE.match(lines[-1])
    assert lines[-1].endswith(" INFO uvicorn.access: GET /health")


def test_modeld_stdout_and_stderr_are_logged_line_by_line(
    tmp_path: Path,
) -> None:
    log_path = tmp_path / "modeld.log"
    state = configure_modeld_logging(
        log_path,
        max_bytes=2048,
        backup_count=2,
        startup_rollover=False,
    )
    try:
        print("stdout one\nstdout two")
        sys.stderr.write("stderr one\nstderr two\n")
        sys.stderr.flush()
    finally:
        state.close()

    lines = log_path.read_text(encoding="utf-8").splitlines()
    assert any(TIMESTAMPED_LINE.match(line) for line in lines)
    assert any(line.endswith(" INFO modeld.stdout: stdout one") for line in lines)
    assert any(line.endswith(" INFO modeld.stdout: stdout two") for line in lines)
    assert any(line.endswith(" WARN modeld.stderr: stderr one") for line in lines)
    assert any(line.endswith(" WARN modeld.stderr: stderr two") for line in lines)


def test_modeld_log_rotates_when_size_limit_is_exceeded(tmp_path: Path) -> None:
    log_path = tmp_path / "modeld.log"
    state = configure_modeld_logging(
        log_path,
        max_bytes=240,
        backup_count=2,
        startup_rollover=False,
    )
    try:
        logger = logging.getLogger("uvicorn.access")
        for index in range(20):
            logger.info("capabilities probe %02d xxxxxxxxxxxxxxxxxxxx", index)
    finally:
        state.close()

    assert log_path.exists()
    assert (tmp_path / "modeld.log.1").exists()
    current = log_path.read_text(encoding="utf-8")
    rotated = (tmp_path / "modeld.log.1").read_text(encoding="utf-8")
    assert "capabilities probe" in current
    assert "capabilities probe" in rotated


def test_modeld_log_rolls_existing_file_on_startup(tmp_path: Path) -> None:
    log_path = tmp_path / "modeld.log"
    log_path.write_text("old modeld log\n", encoding="utf-8")
    (tmp_path / "modeld.log.1").write_text("previous backup\n", encoding="utf-8")

    state = configure_modeld_logging(
        log_path,
        max_bytes=1024,
        backup_count=2,
        startup_rollover=True,
    )
    try:
        logging.getLogger("uvicorn.access").info("new modeld log")
    finally:
        state.close()

    assert (tmp_path / "modeld.log.1").read_text(encoding="utf-8") == "old modeld log\n"
    assert (tmp_path / "modeld.log.2").read_text(encoding="utf-8") == "previous backup\n"
    assert "new modeld log" in log_path.read_text(encoding="utf-8")
