#!/usr/bin/env python3

"""Shared functionality for roundtrip tests."""

from __future__ import annotations

import multiprocessing
import os
import subprocess

cpp_build_dir = "./build/roundtrips"
repo_root = None


def get_repo_root() -> str:
    global repo_root
    if repo_root is not None:
        return repo_root
    else:
        get_rev_parse = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True)
        assert get_rev_parse.returncode == 0
        repo_root = get_rev_parse.stdout.decode("utf-8").strip()
        return repo_root


def run(
    args: list[str], *, env: dict[str, str] | None = None, timeout: int | None = None, cwd: str | None = None
) -> None:
    # Run from the repo root if not specify otherwise.
    if cwd is None:
        cwd = get_repo_root()

    print(f"> {subprocess.list2cmdline(args)}")
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout, check=False, capture_output=True, text=True)
    assert (
        result.returncode == 0
    ), f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"


def roundtrip_env(*, save_path: str | None = None) -> dict[str, str]:
    env = os.environ.copy()

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


def cmake_configure(release: bool, env: dict[str, str]) -> None:
    os.makedirs(cpp_build_dir, exist_ok=True)
    build_type = "Debug"
    if release:
        build_type = "Release"
    # TODO(andreas): We should pixi for the prepare so we can ensure we have build tooling ready
    configure_args = [
        "cmake",
        "-B",
        cpp_build_dir,
        f"-DCMAKE_BUILD_TYPE={build_type}",
        "-DCMAKE_COMPILE_WARNING_AS_ERROR=ON",
        ".",
    ]
    run(
        configure_args,
        env=env,
    )


def cmake_build(target: str, release: bool) -> None:
    config = "Debug"
    if release:
        config = "Release"

    build_process_args = [
        "cmake",
        "--build",
        cpp_build_dir,
        "--config",
        config,
        "--target",
        target,
        "--parallel",
        str(multiprocessing.cpu_count()),
    ]
    run(build_process_args)


def run_comparison(rrd0_path: str, rrd1_path: str, full_dump: bool) -> None:
    cmd = ["rerun", "compare"]
    if full_dump:
        cmd += ["--full-dump"]
    cmd += [rrd0_path, rrd1_path]

    run(cmd, env=roundtrip_env(), timeout=30)
