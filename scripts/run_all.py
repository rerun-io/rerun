#!/usr/bin/env python3

"""Run all examples."""

import argparse
import os
import socket
import subprocess
import sys
import time
from glob import glob
from types import TracebackType
from typing import Any, List, Optional, Tuple, Type


def run_py_example(path: str, viewer_port: int, wait: bool = True, save: Optional[str] = None) -> Any:
    args = ["python3", "main.py", "--num-frames=30", "--steps=200", "--connect", f"--addr=127.0.0.1:{viewer_port}"]
    if save is not None:
        args.append(f"--save={save}")

    process = subprocess.Popen(args, cwd=path)
    if wait:
        returncode = process.wait()
        assert returncode == 0, f"process exited with error code {returncode}"
    return process


def run_saved_example(path: str, wait: bool = True) -> Any:
    process = subprocess.Popen(
        ["cargo", "run", "-p", "rerun", "--all-features", "--", "out.rrd"],
        cwd=path,
    )
    if wait:
        returncode = process.wait()
        assert returncode == 0, f"process exited with error code {returncode}"
    return process


def get_free_port() -> int:
    with socket.socket() as s:
        s.bind(("", 0))
        return int(s.getsockname()[1])


def collect_examples(fast: bool) -> List[str]:
    if fast:
        # cherry picked
        return [
            "examples/python/api_demo",
            "examples/python/car",
            "examples/python/clock",
            "examples/python/colmap",
            "examples/python/deep_sdf",
            "examples/python/dicom",
            "examples/python/nyud",
            "examples/python/plots",
            "examples/python/raw_mesh",
            "examples/python/text_logging",
        ]
    else:
        return [os.path.dirname(entry) for entry in glob("examples/python/**/main.py")]


class Viewer:
    should_close: bool
    web: bool
    sdk_port: int  # where the logging SDK sends the log stream (where the server receives)
    web_viewer_port: int  # the HTTP port where we serve the web viewer
    ws_server_port: int  # the WebSocket port where we serve the log stream
    process: Optional[Any]

    def __init__(self, close: bool = False, web: bool = False):
        self.should_close = close
        self.web = web
        self.sdk_port = get_free_port()
        self.web_viewer_port = get_free_port()
        self.ws_server_port = get_free_port()
        self.process = None

    def close(self) -> None:
        if self.process is not None:
            self.process.kill()
        self.process = None

    def start(self) -> "Viewer":
        args = ["./target/debug/rerun", f"--port={self.sdk_port}"]
        if self.web:
            args += [
                "--web-viewer",
                f"--web-viewer-port={self.web_viewer_port}",
                f"--ws-server-port={self.ws_server_port}",
            ]

        process = subprocess.Popen(
            args,
            bufsize=1,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            universal_newlines=True,
        )
        assert process.stdout is not None

        # wait for process to start
        # we look for the message "Hosting a SDK server over TCP" in its standard output
        started = False
        while not started and process.poll() is None:
            line = process.stdout.readline()
            if "Hosting a SDK server over TCP" in line:
                started = True
            sys.stdout.write(line)

        self.process = process
        return self

    def __enter__(self) -> "Viewer":
        self.start()
        return self

    def __exit__(
        self,
        exc_type: Optional[Type[BaseException]],
        exc: Optional[BaseException],
        traceback: Optional[TracebackType],
    ) -> None:
        if self.process is not None and self.should_close:
            self.process.kill()


def run_sdk_build() -> None:
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


def run_viewer_build() -> None:
    returncode = subprocess.Popen(["cargo", "build", "-p", "rerun", "--all-features"]).wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_web(examples: List[str], separate: bool) -> None:
    if not separate:
        with Viewer(close=True, web=True) as viewer:
            for path in examples:
                run_py_example(path, viewer_port=viewer.sdk_port)
        return

    cleanup: List[Tuple[Any, Any]] = []
    # start all examples in parallel
    for path in examples:
        # each example gets its own viewer
        viewer = Viewer(web=True).start()
        example = run_py_example(path, viewer_port=viewer.sdk_port, wait=False)
        cleanup.append((viewer, example))

    # wait for examples to finish logging
    for pair in cleanup:
        viewer, example = pair
        example.wait()

    # give servers/viewers a moment to finish loading data
    time.sleep(5)

    # shut down servers/viewers
    for pair in cleanup:
        viewer, example = pair
        viewer.close()


def run_save(examples: List[str]) -> None:
    with Viewer(close=True) as viewer:  # ephemeral viewer that exists only while saving
        for example in examples:
            run_py_example(example, viewer_port=viewer.sdk_port, save="out.rrd")


def run_load(examples: List[str], separate: bool, close: bool) -> None:
    if not separate:
        # run all examples sequentially
        for path in examples:
            # each one must be closed for the next one to start running
            run_saved_example(path)
        return

    cleanup: List[Any] = []
    for path in examples:
        process = run_saved_example(path, wait=False)
        cleanup.append(process)

    for process in cleanup:
        process.wait()
        if close:
            process.kill()


def run_native(examples: List[str], separate: bool, close: bool) -> None:
    if not separate:
        # run all examples sequentially in a single viewer
        with Viewer(close) as viewer:
            for path in examples:
                run_py_example(path, viewer_port=viewer.sdk_port, wait=True)
        return

    cleanup: List[Tuple[Any, Any]] = []
    # start all examples in parallel
    for path in examples:
        # each example gets its own viewer
        viewer = Viewer().start()
        example = run_py_example(path, viewer.sdk_port, False)
        cleanup.append((viewer, example))

    # wait for all processes to finish, and close the viewers if requested
    for pair in cleanup:
        viewer, example = pair
        example.wait()
        if close:
            viewer.close()


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
        run_sdk_build()
    run_viewer_build()

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
