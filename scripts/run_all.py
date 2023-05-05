#!/usr/bin/env python3

"""Run all examples."""

import argparse
import os
import socket
import subprocess
import time
from glob import glob
from types import TracebackType
from typing import Any, Callable, List, Optional, Tuple, Type, cast


def run_py_example(path: str, viewer_port: int, wait: bool = True, save: Optional[str] = None) -> Any:
    args = ["--connect", "--addr", f"127.0.0.1:{viewer_port}"]
    if save is not None:
        args += ["--save", str(save)]

    process = subprocess.Popen(
        ["python3", "main.py", "--num-frames=30", "--steps=200"] + args,
        cwd=path,
    )
    if wait:
        returncode = process.wait()
        print(f"process exited with error code {returncode}")
    return process


def run_saved_example(path: str, wait: bool = True) -> Any:
    args = ["out.rrd"]
    process = subprocess.Popen(
        ["cargo", "run", "-p", "rerun", "--all-features", "--"] + args,
        cwd=path,
    )
    if wait:
        returncode = process.wait()
        print(f"process exited with error code {returncode}")
    return process


def get_free_port() -> int:
    with socket.socket() as s:
        s.bind(("", 0))
        return cast(int, s.getsockname()[1])


def collect_examples(fast: bool) -> List[str]:
    if not fast:
        return [os.path.dirname(entry) for entry in glob("examples/python/**/main.py")]
    else:
        # cherry picked
        return [
            "examples/python/deep_sdf",
            "examples/python/dicom",
            "examples/python/raw_mesh",
            "examples/python/clock",
            "examples/python/api_demo",
            "examples/python/car",
            "examples/python/colmap",
            "examples/python/plots",
            "examples/python/nyud",
            "examples/python/text_logging",
        ]


class Viewer:
    port: int
    should_close: bool
    web: bool
    web_viewer_port: int
    ws_server_port: int
    process: Optional[Any]

    def __init__(self, close: bool = False, web: bool = False):
        self.port = get_free_port()
        self.should_close = close
        self.web = web
        self.web_viewer_port = get_free_port()
        self.ws_server_port = get_free_port()
        self.process = None

    def close(self) -> None:
        if self.process is not None:
            self.process.kill()
        self.process = None

    def start(self) -> "Viewer":
        args = ["--port", str(self.port)]
        if self.web:
            args += [
                "--web-viewer",
                "--web-viewer-port",
                str(self.web_viewer_port),
                "--ws-server-port",
                str(self.ws_server_port),
            ]

        self.process = subprocess.Popen(
            ["cargo", "run", "-p", "rerun", "--all-features", "--"] + args,
            stdout=subprocess.PIPE,
        )
        time.sleep(1)  # give it a moment to start
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


def run_build() -> None:
    process = subprocess.Popen(
        ["maturin", "develop", "--manifest-path", "rerun_py/Cargo.toml", '--extras="tests"'],
    )
    returncode = process.wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def run_web(examples: List[str], separate: bool) -> None:
    if separate:
        processes: List[Tuple[Any, Any]] = []
        # start all examples in parallel
        for path in examples:
            # each example gets its own viewer
            viewer = Viewer(web=True).start()
            example = run_py_example(path, viewer_port=viewer.port, wait=False)
            processes.append((viewer, example))
        # wait for all processes to finish, and close the viewers
        for process in processes:
            viewer, example = process
            example.wait()
            viewer.close()
    else:
        with Viewer(close=True, web=True) as viewer:
            for example in examples:
                run_py_example(example, viewer_port=viewer.port)


def run_separate_with(examples: List[str], close: bool, start_example: Callable[[str, int, bool], Any]) -> None:
    processes: List[Tuple[Any, Any]] = []
    # start all examples in parallel
    for path in examples:
        # each example gets its own viewer
        viewer = Viewer().start()
        example = start_example(path, viewer.port, False)
        processes.append((viewer, example))
    # wait for all examples to finish
    for process in processes:
        process[1].wait()
    # close viewers if requested
    if close:
        for process in processes:
            process[0].close()


def run_save(examples: List[str], separate: bool, close: bool) -> None:
    with Viewer(close=True) as viewer:  # ephemeral viewer that exists only while saving
        for example in examples:
            run_py_example(example, viewer_port=viewer.port, save="out.rrd")

    if separate:
        run_separate_with(examples, close, lambda path, viewer_port, wait: run_saved_example(path, wait))
    else:
        with Viewer(close) as viewer:
            for example in examples:
                run_saved_example(example)


def run_native(examples: List[str], separate: bool, close: bool) -> None:
    if separate:
        run_separate_with(examples, close, run_py_example)
    else:
        with Viewer(close) as viewer:
            for example in examples:
                run_py_example(example, viewer_port=viewer.port)


def main() -> None:
    parser = argparse.ArgumentParser(description="Runs all examples.")
    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK.")
    parser.add_argument("--web", action="store_true", help="Run examples in a web viewer.")
    parser.add_argument(
        "--save",
        action="store_true",
        help="Run examples, save them to disk as rrd, then view them natively.",
    )
    parser.add_argument("--fast", action="store_true", help="Run only examples which complete quickly.")
    parser.add_argument("--separate", action="store_true", help="Run each example in a separate viewer.")
    parser.add_argument("--close", action="store_true", help="Close the viewer after running all examples.")

    args = parser.parse_args()

    examples = collect_examples(args.fast)

    if not args.skip_build:
        run_build()

    if args.web:
        run_web(examples, separate=args.separate)
    elif args.save:
        run_save(examples, separate=args.separate, close=args.close)
    else:
        run_native(examples, separate=args.separate, close=args.close)


if __name__ == "__main__":
    main()
