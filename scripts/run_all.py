#!/usr/bin/env python3

"""Run all examples."""
from __future__ import annotations

import argparse
import os
import socket
import subprocess
import time
from glob import glob
from types import TracebackType
from typing import Any

EXTRA_ARGS = {
    "examples/python/clock": ["--steps=200"],  # Make it faster
    "examples/python/live_camera_edge_detection": ["--num-frames=30"],  # Make sure it finishes
    "examples/python/face_tracking": ["--max-frame=30"],  # Make sure it finishes
}

HAS_NO_SAVE_ARG = {
    "examples/python/blueprint",
    "examples/python/dna",
    "examples/python/minimal",
    "examples/python/multiprocessing",
}


def start_process(args: list[str], *, wait: bool, cwd: str | None = None) -> Any:
    process = subprocess.Popen(
        args,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if wait:
        returncode = process.wait()
        if returncode != 0:
            print(process.communicate()[0].decode("utf-8").rstrip())
            print(f"process exited with error code {returncode}")
            exit(returncode)
    return process


def run_py_example(path: str, viewer_port: int | None = None, wait: bool = True, save: str | None = None) -> Any:
    args = ["python3", "main.py"]

    if path in EXTRA_ARGS:
        args += EXTRA_ARGS[path]

    if save is not None:
        args += [f"--save={save}"]
    if viewer_port is not None:
        args += ["--connect", f"--addr=127.0.0.1:{viewer_port}"]

    cmd = " ".join(f'"{a}"' if " " in a else a for a in args)
    print(f"Running example '{path}' via '{cmd}'")

    return start_process(
        args,
        cwd=path,
        wait=wait,
    )


def get_free_port() -> int:
    with socket.socket() as s:
        s.bind(("", 0))
        return int(s.getsockname()[1])


def collect_examples(fast: bool) -> list[str]:
    if fast:
        # cherry picked
        return [
            "examples/python/api_demo",
            "examples/python/car",
            "examples/python/clock",
            "examples/python/dicom_mri",
            "examples/python/plots",
            "examples/python/raw_mesh",
            "examples/python/rgbd",
            "examples/python/signed_distance_fields",
            "examples/python/structure_from_motion",
            "examples/python/text_logging",
        ]
    else:
        skip_list = [
            # depth_sensor requires a specific piece of hardware to be attached
            "examples/python/live_depth_sensor/main.py",
            # objectron currently broken; see https://github.com/rerun-io/rerun/issues/2557
            "examples/python/objectron/main.py",
            # ros requires complex system dependencies to be installed
            "examples/python/ros_node/main.py",
        ]

        return [
            os.path.dirname(main_path) for main_path in glob("examples/python/**/main.py") if main_path not in skip_list
        ]


def print_example_output(path: str, example: Any) -> None:
    print(f"\nExample {path}:\n{example.communicate()[0].decode('utf-8').rstrip()}")


class Viewer:
    should_close: bool
    web: bool
    sdk_port: int  # where the logging SDK sends the log stream (where the server receives)
    web_viewer_port: int  # the HTTP port where we serve the web viewer
    ws_server_port: int  # the WebSocket port where we serve the log stream
    process: Any | None

    def __init__(self, close: bool = False, web: bool = False):
        self.should_close = close
        self.web = web
        self.sdk_port = get_free_port()
        self.web_viewer_port = get_free_port()
        self.ws_server_port = get_free_port()
        self.process = None

    def close(self) -> None:
        if self.process is not None and self.should_close:
            self.process.kill()
        self.process = None

    def start(self) -> Viewer:
        print(f"\nStarting viewer on {'web ' if self.web else ''}port {self.sdk_port}")
        args = ["./target/debug/rerun", f"--port={self.sdk_port}"]
        if self.web:
            args += [
                "--web-viewer",
                f"--web-viewer-port={self.web_viewer_port}",
                f"--ws-server-port={self.ws_server_port}",
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
            "--quiet",
        ],
    ).wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_viewer_build(web: bool) -> None:
    print("Building Rerun Viewer…")
    returncode = subprocess.Popen(
        [
            "cargo",
            "build",
            "-p",
            "rerun-cli",
            "--no-default-features",
            "--features=web_viewer" if web else "--features=native_viewer",
            "--quiet",
        ]
    ).wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_web(examples: list[str], separate: bool) -> None:
    if separate:
        entries: list[tuple[str, Any, Any]] = []
        # start all examples in parallel
        for path in examples:
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
                example = run_py_example(path, viewer_port=viewer.sdk_port)
                print_example_output(path, example)


def run_save(examples: list[str]) -> None:
    for path in examples:
        if path not in HAS_NO_SAVE_ARG:
            example = run_py_example(path, save="out.rrd")
            print_example_output(path, example)


def run_saved_example(path: str, wait: bool) -> Any:
    return start_process(
        ["cargo", "run", "-p", "rerun-cli", "--all-features", "--", os.path.join(path, "out.rrd")],
        wait=wait,
    )


def run_load(examples: list[str], separate: bool, close: bool) -> None:
    if separate:
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
            example = run_saved_example(path, wait=True)
            print_example_output(path, example)


def run_native(examples: list[str], separate: bool, close: bool) -> None:
    if separate:
        # start all examples in parallel:
        cleanup: list[tuple[Any, Any]] = []
        for path in examples:
            # each example gets its own viewer
            viewer = Viewer().start()
            example = run_py_example(path, viewer.sdk_port, False)
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
                example = run_py_example(path, viewer_port=viewer.sdk_port, wait=True)
                print_example_output(path, example)


def main() -> None:
    parser = argparse.ArgumentParser(description="Runs all examples.")
    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK.")
    parser.add_argument("--web", action="store_true", help="Run examples in a web viewer.")
    parser.add_argument(
        "--save",
        action="store_true",
        help="Run examples and save them to disk as rrd.",
    )
    parser.add_argument(
        "--load", action="store_true", help="Run examples using rrd files previously saved via `--save`."
    )
    parser.add_argument("--fast", action="store_true", help="Run only examples which complete quickly.")
    parser.add_argument("--separate", action="store_true", help="Run each example in a separate viewer.")
    parser.add_argument("--close", action="store_true", help="Close the viewer after running all examples.")

    args = parser.parse_args()

    examples = collect_examples(args.fast)

    if not args.skip_build:
        if not args.load:
            run_sdk_build()
        if not args.save:
            run_viewer_build(args.web)

    if args.web:
        run_web(examples, separate=args.separate)
        return

    if args.save:
        run_save(examples)
        if not args.load:
            return

    if args.load:
        run_load(examples, separate=args.separate, close=args.close)
        return

    run_native(examples, separate=args.separate, close=args.close)


if __name__ == "__main__":
    main()
