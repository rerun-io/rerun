#!/usr/bin/env python3

"""Shared functionality for roundtrip tests."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


def get_rerun_root() -> str:
    # Search upward for .RERUN_ROOT sentinel file
    # TODO(RR-3355): Use a shared utility for this
    current = Path(__file__).resolve().parent
    while current != current.parent:
        if (current / ".RERUN_ROOT").exists():
            return str(current)
        current = current.parent
    raise FileNotFoundError("Could not find .RERUN_ROOT sentinel file in any parent directory")


def run(
    args: list[str],
    *,
    env: dict[str, str] | None = None,
    timeout: int | None = None,
    cwd: str | None = None,
) -> None:
    # Run from the rerun root if not specify otherwise.
    if cwd is None:
        cwd = get_rerun_root()

    print(f"> {subprocess.list2cmdline(args)}")
    result = subprocess.run(
        args,
        env=env,
        cwd=cwd,
        timeout=timeout,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    assert result.returncode == 0, (
        f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"
    )


def roundtrip_env(*, save_path: str | None = None) -> dict[str, str]:
    env = os.environ.copy()

    # raise exception on warnings, e.g. when using a @deprecated function:
    env["PYTHONWARNINGS"] = "error"

    # NOTE: Make sure to disable batching, otherwise the Arrow concatenation logic within
    # the batcher will happily insert uninitialized padding bytes as needed!
    env["RERUN_FLUSH_NUM_ROWS"] = "0"

    # Turn on strict mode to catch errors early
    env["RERUN_STRICT"] = "1"

    # Treat any warning as panics
    env["RERUN_PANIC_ON_WARN"] = "1"

    if save_path:
        # NOTE: Force the recording stream to write to disk!
        env["_RERUN_TEST_FORCE_SAVE"] = save_path

    return env


def run_comparison(rrd0_path: str, rrd1_path: str, full_dump: bool) -> None:
    cmd = ["rerun", "rrd", "compare", "--unordered", "--ignore-chunks-without-components"]
    if full_dump:
        cmd += ["--full-dump"]
    cmd += [rrd0_path, rrd1_path]

    run(cmd, env=roundtrip_env(), timeout=60)
