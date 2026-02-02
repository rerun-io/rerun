#!/usr/bin/env python3

"""
Generate comparison between examples and their related screenshots.

This script builds/gather RRDs and corresponding screenshots and displays
them side-by-side. It pulls from the following sources:

- The screenshots listed in .fbs files (crates/store/re_sdk_types/definitions/rerun/**/*.fbs),
  and the corresponding snippets in the docs (docs/snippets/*.rs)
- The `rerun.io/viewer` examples, as built by the `re_dev_tools`/`build_examples` script.

The comparisons are generated in the `compare_screenshot` directory. Use the `--serve`
option to show them in a browser.
"""

from __future__ import annotations

import argparse
import http.server
import json
import multiprocessing
import os
import shutil
import subprocess
import threading
from dataclasses import dataclass
from functools import partial
from io import BytesIO
from pathlib import Path
from typing import TYPE_CHECKING, Any

import requests
from jinja2 import Template
from PIL import Image

if TYPE_CHECKING:
    from collections.abc import Iterable

BASE_PATH = Path("compare_screenshot")


SCRIPT_DIR_PATH = Path(__file__).parent
STATIC_ASSETS = SCRIPT_DIR_PATH / "assets" / "static"
TEMPLATE_DIR = SCRIPT_DIR_PATH / "assets" / "templates"
INDEX_TEMPLATE = Template((TEMPLATE_DIR / "index.html").read_text())
EXAMPLE_TEMPLATE = Template((TEMPLATE_DIR / "example.html").read_text())
RERUN_DIR = SCRIPT_DIR_PATH.parent.parent
SNIPPETS_DIR = RERUN_DIR / "docs" / "snippets"


def measure_thumbnail(url: str) -> Any:
    """Downloads `url` and returns its width and height."""
    response = requests.get(url)
    response.raise_for_status()
    image = Image.open(BytesIO(response.content))
    return image.size


def run(
    args: list[str],
    *,
    env: dict[str, str] | None = None,
    timeout: int | None = None,
    cwd: str | Path | None = None,
) -> None:
    print(f"> {subprocess.list2cmdline(args)}")
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout, check=False, capture_output=True, text=True)
    assert result.returncode == 0, (
        f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"
    )


@dataclass
class Example:
    name: str
    title: str
    rrd: Path
    screenshot_url: str


def copy_static_assets(examples: list[Example]) -> None:
    # copy root
    dst = BASE_PATH
    print(f"\nCopying static assets from {STATIC_ASSETS} to {dst}")
    shutil.copytree(STATIC_ASSETS, dst, dirs_exist_ok=True)

    # copy examples
    for example in examples:
        dst = BASE_PATH / "examples" / example.name
        shutil.copytree(
            STATIC_ASSETS,
            dst,
            dirs_exist_ok=True,
            ignore=shutil.ignore_patterns("index.html"),
        )


def build_python_sdk() -> None:
    print("Building Python SDK…")
    run(["pixi", "run", "py-build", "--features", "web_viewer"])


# ====================================================================================================
# SNIPPETS
#
# We scrape FBS for screenshot URL and generate the corresponding snippets RRD with compare_snippet_output.py
# ====================================================================================================


def extract_snippet_urls_from_fbs() -> dict[str, str]:
    fbs_path = SCRIPT_DIR_PATH.parent.parent / "crates" / "store" / "re_sdk_types" / "definitions" / "rerun"

    urls = {}
    for fbs in fbs_path.glob("**/*.fbs"):
        for line in fbs.read_text().splitlines():
            if line.startswith(r"/// \example"):
                name = line.split()[2]

                idx = line.find('image="')
                if idx != -1:
                    end_idx = line.find('"', idx + 8)
                    if end_idx == -1:
                        end_idx = len(line)
                    urls[name] = line[idx + 7 : end_idx]

    return urls


SNIPPET_URLS = extract_snippet_urls_from_fbs()


def build_snippets() -> None:
    cmd = [
        str(SNIPPETS_DIR / "compare_snippet_output.py"),
        "--no-py",
        "--no-cpp",
        "--no-py-build",
        "--no-cpp-build",
    ]

    for name in SNIPPET_URLS.keys():
        run([*cmd, name], cwd=RERUN_DIR)


def collect_snippets() -> Iterable[Example]:
    for name in sorted(SNIPPET_URLS.keys()):
        rrd = SNIPPETS_DIR / "all" / f"{name}_rust.rrd"
        if rrd.exists():
            yield Example(name=name, title=name, rrd=rrd, screenshot_url=SNIPPET_URLS[name])
        else:
            print(f"WARNING: Missing {rrd} for {name}")


# ====================================================================================================
# DEMO EXAMPLES
#
# We run the `re_dev_tools`/`build_examples` script and scrap the output "example_data" directory.
# ====================================================================================================


def build_examples() -> None:
    # fmt: off
    cmd = [
        "pixi", "run", "-e", "examples",
        "cargo", "run", "--locked",
        "-p", "re_dev_tools", "--",
        "build-examples", "rrd", "example_data",
        # TODO(andreas): nightly channel would be better, but the dependencies that requires make things hard to get to run.
        "--channel", "main",
    ]
    run(cmd, cwd=RERUN_DIR)

    cmd = [
        "pixi", "run", "-e", "examples",
        "cargo", "run", "--locked",
        "-p", "re_dev_tools", "--",
        "build-examples", "manifest", "example_data/examples_manifest.json",
        # TODO(andreas): nightly channel would be better, but the dependencies that requires make things hard to get to run.
        "--channel", "main",
    ]
    run(cmd, cwd=RERUN_DIR)
    # fmt: on


def collect_examples() -> Iterable[Example]:
    example_dir = RERUN_DIR / "example_data"
    assert example_dir.exists(), "Examples have not been built yet."

    manifest = json.loads((example_dir / "examples_manifest.json").read_text())

    for example in manifest:
        name = example["name"]
        rrd = example_dir / f"{name}.rrd"
        assert rrd.exists(), f"Missing {rrd} for {name}"

        yield Example(
            name=name,
            title=example["title"],
            rrd=rrd,
            screenshot_url=example["thumbnail"]["url"],
        )


def render_index(examples: list[Example]) -> None:
    BASE_PATH.mkdir(exist_ok=True)

    index_path = BASE_PATH / "index.html"
    print(f"Rendering index.html -> {index_path}")
    index_path.write_text(INDEX_TEMPLATE.render(examples=examples))


def render_examples(examples: list[Example]) -> None:
    print("Rendering examples")

    for example in examples:
        target_path = BASE_PATH / "examples" / example.name
        target_path.mkdir(parents=True, exist_ok=True)
        index_path = target_path / "index.html"
        print(f"{example.name} -> {index_path}")
        index_path.write_text(EXAMPLE_TEMPLATE.render(example=example, examples=examples))

        shutil.copy(example.rrd, target_path / "data.rrd")


class CORSRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self) -> None:
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()


def serve_files() -> None:
    def serve() -> None:
        print("\nServing examples at http://127.0.0.1:8080/\n")
        server = http.server.HTTPServer(
            server_address=("127.0.0.1", 8080),
            RequestHandlerClass=partial(
                CORSRequestHandler,
                directory=str(BASE_PATH),
            ),
        )
        server.serve_forever()

    def serve_rerun() -> None:
        import rerun as rr

        os.environ["RUST_LOG"] = "rerun=warn"

        rr.init("rerun_example_screenshot_compare")
        connect_to = rr.serve_grpc()
        rr.serve_web_viewer(open_browser=False, connect_to=connect_to)

    threading.Thread(target=serve, daemon=True).start()

    # use a sub-process so the target can change env variables without affecting the parent
    process = multiprocessing.Process(target=serve_rerun, daemon=True)
    process.run()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--serve",
        action="store_true",
        help="Serve the app on this port after building [default: 8080]",
    )
    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK.")
    parser.add_argument("--skip-example-build", action="store_true", help="Skip building the RRDs.")

    args = parser.parse_args()

    if not args.skip_build:
        build_python_sdk()

    if not args.skip_example_build:
        build_snippets()
        build_examples()

    examples = list(collect_snippets()) + list(collect_examples())
    assert len(examples) > 0, "No examples found"

    render_index(examples)
    render_examples(examples)
    copy_static_assets(examples)

    if args.serve:
        serve_files()

        while True:
            try:
                print("Press enter to reload static files")
                input()
                render_examples(examples)
                copy_static_assets(examples)
            except KeyboardInterrupt:
                break


if __name__ == "__main__":
    main()
