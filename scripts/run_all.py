#!/usr/bin/env python3

"""Run all examples."""

from __future__ import annotations

import argparse
import os
import socket
import subprocess
import sys
import time
from glob import glob
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from types import TracebackType

EXTRA_ARGS = {
    "examples/python/clock": ["--steps=200"],  # Make it faster
    "examples/python/detect_and_track_objects": ["--max-frame=10"],  # Make it faster
    "examples/python/face_tracking": ["--max-frame=30"],  # Make sure it finishes
    "examples/python/human_pose_tracking": ["--max-frame=10"],  # Make it faster
    "examples/python/live_camera_edge_detection": ["--num-frames=30"],  # Make sure it finishes
}

HAS_NO_RERUN_ARGS = {
    "examples/python/blueprint",
    "examples/python/dna",
    "examples/python/minimal",
    "examples/python/multiprocessing",
    "examples/python/shared_recording",
    "examples/python/stdio",
}

MIN_PYTHON_REQUIREMENTS: dict[str, tuple[int, int]] = {
    "examples/python/controlnet": (3, 10),
    # pyopf requires Python 3.10
    "examples/python/open_photogrammetry_format": (3, 10),
}

MAX_PYTHON_REQUIREMENTS: dict[str, tuple[int, int]] = {
    "examples/python/face_tracking": (3, 11),  # TODO(ab): remove when mediapipe is 3.12 compatible
    "examples/python/human_pose_tracking": (3, 11),  # TODO(ab): remove when mediapipe is 3.12 compatible
    "examples/python/llm_embedding_ner": (3, 11),  # TODO(ab): remove when torch is umap-learn/numba is 3.12 compatible
}

SKIP_LIST = [
    # depth_sensor requires a specific piece of hardware to be attached
    "examples/python/live_depth_sensor",
    # ros requires complex system dependencies to be installed
    "examples/python/ros_node",
    # this needs special treatment to run
    "examples/python/external_data_loader",
]

MAC_SKIP_LIST: list[str] = []


def start_process(args: list[str], *, wait: bool) -> Any:
    readable_cmd = " ".join(f'"{a}"' if " " in a else a for a in args)
    print(f"> {readable_cmd}")

    process = subprocess.Popen(
        args,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if wait:
        returncode = process.wait()
        if returncode != 0:
            print(output_from_process(process))
            print()
            print(f"process exited with error code {returncode}")
            sys.exit(returncode)
    return process


def run_py_example(path: str, viewer_port: int | None = None, *, wait: bool = True, save: str | None = None) -> Any:
    args = [os.path.join(path, "main.py")]

    if path in EXTRA_ARGS:
        args += EXTRA_ARGS[path]

    if save is not None:
        args += [f"--save={save}"]
    if viewer_port is not None:
        args += ["--connect", f"--url=rerun+http://127.0.0.1:{viewer_port}/proxy"]

    return start_process(
        args,
        wait=wait,
    )


# stdout and stderr
def output_from_process(process: subprocess.Popen[bytes]) -> str:
    return process.communicate()[0].decode("utf-8").rstrip()


def get_free_port() -> int:
    with socket.socket() as s:
        s.bind(("", 0))
        return int(s.getsockname()[1])


def collect_examples(fast: bool) -> list[str]:
    if fast:
        # cherry-picked
        return [
            "examples/python/car",
            "examples/python/clock",
            "examples/python/dicom_mri",
            "examples/python/plots",
            "examples/python/raw_mesh",
            "examples/python/rgbd",
            "examples/python/structure_from_motion",
        ]
    else:
        examples = []
        for main_path in glob("examples/python/**/main.py"):
            example = os.path.dirname(main_path)

            if example in SKIP_LIST:
                continue

            major, minor, *_ = sys.version_info

            if example in MIN_PYTHON_REQUIREMENTS:
                req_major, req_minor = MIN_PYTHON_REQUIREMENTS[example]
                if major < req_major or (major == req_major and minor < req_minor):
                    continue

            if example in MAX_PYTHON_REQUIREMENTS:
                req_major, req_minor = MAX_PYTHON_REQUIREMENTS[example]
                if major > req_major or (major == req_major and minor > req_minor):
                    continue

            if example in MAC_SKIP_LIST and sys.platform == "darwin":
                continue

            examples.append(example)

        return examples


def print_example_output(path: str, example: Any) -> None:
    output = example.communicate()[0].decode("utf-8").rstrip()
    print(f"\nExample {path}:\n{output}\n")


class Viewer:
    should_close: bool
    web: bool
    sdk_port: int  # where the logging SDK sends the log stream (where the server receives)
    web_viewer_port: int  # the HTTP port where we serve the web viewer
    grpc_server_port: int  # the gRPC port where we serve the log stream
    process: Any | None

    def __init__(self, close: bool = False, web: bool = False) -> None:
        self.should_close = close
        self.web = web
        self.sdk_port = get_free_port()
        self.web_viewer_port = get_free_port()
        self.grpc_server_port = get_free_port()
        self.process = None

    def close(self) -> None:
        if self.process is not None and self.should_close:
            self.process.kill()
        self.process = None

    def start(self) -> Viewer:
        print(f"\nStarting viewer on {'web ' if self.web else ''}port {self.sdk_port}")
        CARGO_TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", "./target"))
        args = [f"{CARGO_TARGET_DIR}/debug/rerun", f"--port={self.sdk_port}"]
        if self.web:
            args += [
                "--web-viewer",
                f"--web-viewer-port={self.web_viewer_port}",
                f"--port={self.grpc_server_port}",
            ]

        self.process = subprocess.Popen(args)
        time.sleep(1)
        return self

    def __enter__(self) -> Viewer:
        self.start()
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.close()


def run_sdk_build() -> None:
    print("Building Python SDK…")
    returncode = subprocess.Popen(
        [
            "maturin",
            "develop",
            "--manifest-path",
            "rerun_py/Cargo.toml",
            '--extras="tests"',
        ],
    ).wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_viewer_build(web: bool) -> None:
    print("Building Rerun Viewer…")
    returncode = subprocess.Popen([
        "cargo",
        "build",
        "-p",
        "rerun-cli",
        "--no-default-features",
        "--features=web_viewer" if web else "--features=native_viewer",
    ]).wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_install_requirements(examples: list[str]) -> None:
    """Install dependencies for the provided list of examples if they have a requirements.txt file."""
    args = []
    for example in examples:
        req = Path(example) / "requirements.txt"
        if req.exists():
            args.extend(["-r", str(req)])

    print("Installing examples requirements…")
    returncode = subprocess.Popen(["pip", "install", *args]).wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_web(examples: list[str], parallel: bool) -> None:
    if parallel:
        entries: list[tuple[str, Any, Any]] = []
        # start all examples in parallel
        for path in examples:
            if path in HAS_NO_RERUN_ARGS:
                continue
            # each example gets its own viewer
            viewer = Viewer(web=True).start()
            example = run_py_example(path, viewer_port=viewer.sdk_port, wait=False)
            entries.append((path, viewer, example))

        # wait for examples to finish logging
        for entry in entries:
            _, _, example = entry
            example.wait()

        # give servers/viewers a moment to finish loading data
        time.sleep(5)

        # shut down servers/viewers
        for entry in entries:
            path, viewer, example = entry
            print_example_output(path, example)
            viewer.close()

    else:
        with Viewer(close=True, web=True) as viewer:
            for path in examples:
                if path in HAS_NO_RERUN_ARGS:
                    continue
                process = run_py_example(path, viewer_port=viewer.sdk_port)
                print(f"{output_from_process(process)}\n")


def run_save(examples: list[str]) -> None:
    for path in examples:
        if path not in HAS_NO_RERUN_ARGS:
            process = run_py_example(path, save=os.path.join(path, "out.rrd"))
            print(f"{output_from_process(process)}\n")


def run_saved_example(path: str, *, wait: bool) -> Any:
    return start_process(
        [
            "cargo",
            "rerun",
            os.path.join(path, "out.rrd"),
        ],
        wait=wait,
    )


def run_load(examples: list[str], parallel: bool, close: bool) -> None:
    examples = [path for path in examples if path not in HAS_NO_RERUN_ARGS]

    if parallel:
        entries: list[tuple[str, Any]] = []
        for path in examples:
            example = run_saved_example(path, wait=False)
            entries.append((path, example))

        for entry in entries:
            path, example = entry
            print_example_output(path, example)
            if close:
                example.kill()
    else:
        # run all examples sequentially
        for path in examples:
            # each one must be closed for the next one to start running
            process = run_saved_example(path, wait=True)
            print(f"{output_from_process(process)}\n")


def run_native(examples: list[str], parallel: bool, close: bool) -> None:
    if parallel:
        # start all examples in parallel:
        cleanup: list[tuple[Any, Any]] = []
        for path in examples:
            if path in HAS_NO_RERUN_ARGS:
                continue
            # each example gets its own viewer
            viewer = Viewer().start()
            example = run_py_example(path, viewer.sdk_port, wait=False)
            cleanup.append((viewer, example))

        # wait for all processes to finish, and close the viewers if requested
        for pair in cleanup:
            viewer, example = pair
            print_example_output(path, example)
            if close:
                viewer.close()
    else:
        # run all examples sequentially in a single viewer
        with Viewer(close) as viewer:
            for path in examples:
                if path in HAS_NO_RERUN_ARGS:
                    continue
                process = run_py_example(path, viewer_port=viewer.sdk_port, wait=True)
                print(f"{output_from_process(process)}\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Runs all examples.")
    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK.")
    parser.add_argument(
        "--install-requirements",
        action="store_true",
        help="Install Python requirements for each example.",
    )
    parser.add_argument("--web", action="store_true", help="Run examples in a web viewer.")
    parser.add_argument(
        "--save",
        action="store_true",
        help="Run examples and save them to disk as rrd.",
    )
    parser.add_argument(
        "--load",
        action="store_true",
        help="Run examples using rrd files previously saved via `--save`.",
    )
    parser.add_argument("--fast", action="store_true", help="Run only examples which complete quickly.")
    parser.add_argument("--parallel", action="store_true", help="Run all examples in parallel.")
    parser.add_argument("--close", action="store_true", help="Close the viewer after running all examples.")

    args = parser.parse_args()

    examples = collect_examples(args.fast)

    if not args.skip_build:
        if not args.load:
            run_sdk_build()
        if not args.save:
            run_viewer_build(args.web)

    if args.install_requirements:
        run_install_requirements(examples)

    if args.web:
        run_web(examples, parallel=args.parallel)
        return

    if args.save:
        run_save(examples)
        if not args.load:
            return

    if args.load:
        run_load(examples, parallel=args.parallel, close=args.close)
        return

    run_native(examples, parallel=args.parallel, close=args.close)


if __name__ == "__main__":
    main()
