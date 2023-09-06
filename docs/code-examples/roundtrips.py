#!/usr/bin/env python3

"""Run end-to-end cross-language roundtrip tests for all code examples."""

from __future__ import annotations

import argparse
import multiprocessing
import os
import subprocess
import sys
import time
from os import listdir
from os.path import isfile, join

# fmt: off
opt_out_entirely = {
    "annotation_context_arrows": ["py", "rust"], # TODO(#3207): should be rects, but not available in cpp yet
    "annotation_context_connections": ["cpp"],
    "annotation_context_rects": ["cpp"],
    "annotation_context_segmentation": ["cpp"],
    "box3d_batch": ["cpp", "rust"],
    "box3d_simple": ["cpp"],
    "depth_image_3d": ["cpp"],
    "depth_image_simple": ["cpp"],
    "image_advanced": ["cpp", "rust"],
    "image_simple": ["cpp"],
    "line_segments2d_simple": ["cpp"], # TODO(#2786): working nicely, but we need rect2d bounds!
    "line_strip2d_batch": ["cpp"], # TODO(#2786): working nicely, but we need rect2d bounds!
    "line_strip2d_simple": ["cpp"], # TODO(#2786): working nicely, but we need rect2d bounds!
    "mesh_simple": ["cpp"],
    "pinhole_simple": ["cpp", "py", "rust"], # TODO(#3206): need to align everything to use PCG64 in the same order etc... don't have time for that.
    "point2d_random": ["cpp", "py", "rust"], # TODO(#3206): need to align everything to use PCG64 in the same order etc... don't have time for that.
    "point2d_simple": ["cpp"],
    "point3d_random": ["cpp", "py", "rust"], # TODO(#3206): need to align everything to use PCG64 in the same order etc... don't have time for that.
    "rect2d_simple": ["cpp"],
    "scalar_simple": ["cpp", "py", "rust"], # TODO(#3206): need to align everything to use PCG64 in the same order etc... don't have time for that.
    "segmentation_image_simple": ["cpp"],
    "tensor_one_dim": ["cpp", "py", "rust"], # TODO(#3206): need to align everything to use PCG64 in the same order etc... don't have time for that.
    "tensor_simple": ["cpp", "py", "rust"], # TODO(#3206): need to align everything to use PCG64 in the same order etc... don't have time for that.
    "text_log_integration": ["cpp", "rust"],
    "text_entry_simple": ["cpp"],
    # TODO(#3207): two issues:
    #   - cpp sends an identity transform for no reason
    #   - python has a crazy indicator component out of nowhere
    "transform3d_simple": ["cpp", "py", "rust"],

    # This is this script, it's not an example.
    "roundtrips": ["cpp", "py", "rust"],
}
# fmt: on


def main() -> None:
    parser = argparse.ArgumentParser(description="Run end-to-end cross-language roundtrip tests for all API examples")
    parser.add_argument("--no-py-build", action="store_true", help="Skip building rerun-sdk for Python")
    parser.add_argument(
        "--no-cpp-build",
        action="store_true",
        help="Skip cmake configure and ahead of time build for rerun_c & rerun_cpp",
    )
    parser.add_argument("--full-dump", action="store_true", help="Dump both rrd files as tables")
    parser.add_argument(
        "--release",
        action="store_true",
        help="Run cargo invocations with --release and CMake with `-DCMAKE_BUILD_TYPE=Release` & `--config Release`",
    )
    parser.add_argument("--target", type=str, default=None, help="Target used for cargo invocations")
    parser.add_argument("--target-dir", type=str, default=None, help="Target directory used for cargo invocations")
    parser.add_argument("example", nargs="*", type=str, default=None, help="Run only the specified examples")

    args = parser.parse_args()

    build_env = os.environ.copy()
    if "RUST_LOG" in build_env:
        del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

    if args.no_py_build:
        print("Skipping building python rerun-sdk - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Building rerun-sdk for Python…")
        start_time = time.time()
        returncode = subprocess.Popen(["just", "py-build", "--quiet"], env=build_env).wait()
        assert returncode == 0, f"Python rerun-sdk build failed with exit code {returncode}"
        elapsed = time.time() - start_time
        print(f"rerun-sdk for Python built in {elapsed:.1f} seconds")
        print("")

    if args.no_cpp_build:
        print("Skipping cmake configure & build for rerun_c & rerun_cpp - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Build rerun_c & rerun_cpp…")
        start_time = time.time()
        os.makedirs("build", exist_ok=True)
        build_type = "Debug"
        if args.release:
            build_type = "Release"
        configure_args = ["cmake", f"-DCMAKE_BUILD_TYPE={build_type}", "-DCMAKE_COMPILE_WARNING_AS_ERROR=ON", ".."]
        print("> ${subprocess.list2cmdline(configure_args)}")
        returncode = subprocess.Popen(
            configure_args,
            env=build_env,
            cwd="build",
        ).wait()
        assert returncode == 0, f"configuring cmake failed with exit code {returncode}"
        cmake_build("rerun_sdk", args.release)
        elapsed = time.time() - start_time
        print(f"rerun-sdk for C++ built in {elapsed:.1f} seconds")
        print("")

    if len(args.example) > 0:
        examples = args.example
    else:
        dir = os.path.dirname(__file__)
        files = [f for f in listdir(dir) if isfile(join(dir, f))]
        examples = [
            filename
            for filename, extension in [os.path.splitext(file) for file in files]
            if extension in (".cpp", ".py", ".rs")
        ]

    examples = list(set(examples))
    examples.sort()

    for example in examples:
        example_opt_out = opt_out_entirely.get(example, [])

        if "rust" not in example_opt_out:
            rust_output_path = run_roundtrip_rust(example, args.release, args.target, args.target_dir)

            if "py" not in example_opt_out:
                python_output_path = run_roundtrip_python(example)
                run_comparison(python_output_path, rust_output_path, args.full_dump)

            if "cpp" not in example_opt_out:
                cpp_output_path = run_roundtrip_cpp(example, args.release)
                run_comparison(rust_output_path, cpp_output_path, args.full_dump)


def roundtrip_env(*, save_path: str | None = None) -> dict[str, str]:
    # NOTE: Make sure to disable batching, otherwise the Arrow concatenation logic within
    # the batcher will happily insert uninitialized padding bytes as needed!
    env = os.environ.copy()
    env["RERUN_FLUSH_NUM_ROWS"] = "0"

    if save_path:
        # NOTE: Force the recording stream to write to disk!
        env["_RERUN_TEST_FORCE_SAVE"] = save_path

    return env


def run_roundtrip_python(example: str) -> str:
    main_path = f"docs/code-examples/{example}.py"
    output_path = f"docs/code-examples/{example}_py.rrd"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    cmd = [python_executable, main_path]

    env = roundtrip_env(save_path=output_path)
    print(f"\n> _RERUN_TEST_FORCE_SAVE={env['_RERUN_TEST_FORCE_SAVE']} {subprocess.list2cmdline(cmd)}")
    roundtrip_process = subprocess.Popen(cmd, env=env)
    returncode = roundtrip_process.wait(timeout=30)
    assert returncode == 0, f"python roundtrip process exited with error code {returncode}"

    return output_path


def run_roundtrip_rust(example: str, release: bool, target: str | None, target_dir: str | None) -> str:
    output_path = f"docs/code-examples/{example}_rust.rrd"

    cmd = ["cargo", "run", "--quiet", "-p", "code_examples", "--bin", example]

    if target is not None:
        cmd += ["--target", target]

    if target_dir is not None:
        cmd += ["--target-dir", target_dir]

    if release:
        cmd += ["--release"]

    env = roundtrip_env(save_path=output_path)
    print(f"\n> _RERUN_TEST_FORCE_SAVE={env['_RERUN_TEST_FORCE_SAVE']} {subprocess.list2cmdline(cmd)}")
    roundtrip_process = subprocess.Popen(cmd, env=env)
    returncode = roundtrip_process.wait(timeout=12000)
    assert returncode == 0, f"rust roundtrip process exited with error code {returncode}"

    return output_path


def run_roundtrip_cpp(example: str, release: bool) -> str:
    target_name = f"{example}"
    output_path = f"docs/code-examples/{example}_cpp.rrd"

    cmake_build(target_name, release)

    cmd = [f"./build/docs/code-examples/{example}"]
    env = roundtrip_env(save_path=output_path)
    print(f"\n> _RERUN_TEST_FORCE_SAVE={env['_RERUN_TEST_FORCE_SAVE']} {subprocess.list2cmdline(cmd)}")
    roundtrip_process = subprocess.Popen(cmd, env=env)
    returncode = roundtrip_process.wait(timeout=12000)
    assert returncode == 0, f"cpp roundtrip process exited with error code {returncode}"

    return output_path


def cmake_build(target: str, release: bool) -> None:
    config = "Debug"
    if release:
        config = "Release"

    build_process_args = [
        "cmake",
        "--build",
        ".",
        "--config",
        config,
        "--target",
        target,
        "--parallel",
        str(multiprocessing.cpu_count()),
    ]
    print(f"\n> {subprocess.list2cmdline(build_process_args)}")
    result = subprocess.run(build_process_args, cwd="build")

    assert result.returncode == 0, f"cmake build of {target} exited with error code {result.returncode}"


def run_comparison(rrd0_path: str, rrd1_path: str, full_dump: bool) -> None:
    cmd = ["rerun", "compare"]
    if full_dump:
        cmd += ["--full-dump"]
    cmd += [rrd0_path, rrd1_path]

    print(f"\n> {subprocess.list2cmdline(cmd)}")
    comparison_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = comparison_process.wait(timeout=30)
    assert returncode == 0, f"comparison process exited with error code {returncode}"


if __name__ == "__main__":
    main()
