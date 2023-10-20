#!/usr/bin/env python3

"""
Generate comparison between examples and their related screenshots.

This script builds/gather RRDs and corresponding screenshots and displays
them side-by-side. It pulls from the following sources:

- The screenshots listed in .fbs files (crates/re_types/definitions/rerun/**/*.fbs),
  and the corresponding code examples in the docs (docs/code-examples/*.rs)
- The `demo.rerun.io` examples, as built by the `build_demo_app.py` script.

The comparisons are generated in the `compare_screenshot` directory. Use the `--serve`
option to show them in a browser.
"""

from __future__ import annotations

import argparse
import http.server
import json
import os
import shutil
import subprocess
import threading
from dataclasses import dataclass
from functools import partial
from io import BytesIO
from pathlib import Path
from typing import Any, Iterable

import requests
from jinja2 import Template
from PIL import Image

BASE_PATH = Path("compare_screenshot")


SCRIPT_DIR_PATH = Path(__file__).parent
STATIC_ASSETS = SCRIPT_DIR_PATH / "assets" / "static"
TEMPLATE_DIR = SCRIPT_DIR_PATH / "assets" / "templates"
INDEX_TEMPLATE = Template((TEMPLATE_DIR / "index.html").read_text())
EXAMPLE_TEMPLATE = Template((TEMPLATE_DIR / "example.html").read_text())
RERUN_DIR = SCRIPT_DIR_PATH.parent.parent
CODE_EXAMPLE_DIR = RERUN_DIR / "docs" / "code-examples"


def measure_thumbnail(url: str) -> Any:
    """Downloads `url` and returns its width and height."""
    response = requests.get(url)
    response.raise_for_status()
    image = Image.open(BytesIO(response.content))
    return image.size


def run(
    args: list[str], *, env: dict[str, str] | None = None, timeout: int | None = None, cwd: str | Path | None = None
) -> None:
    print(f"> {subprocess.list2cmdline(args)}")
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout, check=False, capture_output=True, text=True)
    assert (
        result.returncode == 0
    ), f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"


@dataclass
class Example:
    name: str
    title: str
    rrd: Path
    screenshot_url: str
    description_html: str = ""
    source_url: str = ""


def copy_static_assets(examples: list[Example]) -> None:
    # copy root
    dst = BASE_PATH
    print(f"\nCopying static assets from {STATIC_ASSETS} to {dst}")
    shutil.copytree(STATIC_ASSETS, dst, dirs_exist_ok=True)

    # copy examples
    for example in examples:
        dst = os.path.join(BASE_PATH, f"examples/{example.name}")
        shutil.copytree(
            STATIC_ASSETS,
            dst,
            dirs_exist_ok=True,
            ignore=shutil.ignore_patterns("index.html"),
        )


def build_python_sdk() -> None:
    print("Building Python SDK…")
    run(
        [
            "maturin",
            "develop",
            "--manifest-path",
            "rerun_py/Cargo.toml",
            '--extras="tests"',
            "--quiet",
        ]
    )


def build_wasm() -> None:
    print("")
    run(["cargo", "r", "-p", "re_build_web_viewer", "--", "--release"])


def copy_wasm(examples: list[Example]) -> None:
    files = ["re_viewer_bg.wasm", "re_viewer.js"]
    for example in examples:
        for file in files:
            shutil.copyfile(
                os.path.join("web_viewer", file),
                os.path.join(BASE_PATH, f"examples/{example.name}", file),
            )


# ====================================================================================================
# CODE EXAMPLES
#
# We scrape FBS for screenshot URL and generate the corresponding code examples RRD with roundtrips.py
# ====================================================================================================


def extract_code_example_urls_from_fbs() -> dict[str, str]:
    fbs_path = SCRIPT_DIR_PATH.parent.parent / "crates" / "re_types" / "definitions" / "rerun"

    urls = {}
    for fbs in fbs_path.glob("**/*.fbs"):
        for line in fbs.read_text().splitlines():
            if line.startswith(r"/// \example"):
                if "!api" in line:
                    continue

                name = line.split()[2]

                idx = line.find('image="')
                if idx != -1:
                    end_idx = line.find('"', idx + 8)
                    if end_idx == -1:
                        end_idx = len(line)
                    urls[name] = line[idx + 7 : end_idx]

    return urls


CODE_EXAMPLE_URLS = extract_code_example_urls_from_fbs()


def build_code_examples() -> None:
    cmd = [
        str(CODE_EXAMPLE_DIR / "roundtrips.py"),
        "--no-py",
        "--no-cpp",
        "--no-py-build",
        "--no-cpp-build",
    ]

    for name in CODE_EXAMPLE_URLS.keys():
        run(cmd + [name], cwd=RERUN_DIR)


def collect_code_examples() -> Iterable[Example]:
    for name in sorted(CODE_EXAMPLE_URLS.keys()):
        rrd = CODE_EXAMPLE_DIR / f"{name}_rust.rrd"
        assert rrd.exists(), f"Missing {rrd} for {name}"
        yield Example(name=name, title=name, rrd=rrd, screenshot_url=CODE_EXAMPLE_URLS[name])


# ====================================================================================================
# DEMO EXAMPLES
#
# We run the `build_demo_app.py` script and scrap the output "web_demo" directory.
# ====================================================================================================


BUILD_DEMO_APP_SCRIPT = RERUN_DIR / "scripts" / "ci" / "build_demo_app.py"


def build_demo_examples(skip_example_build: bool = False) -> None:
    cmd = [
        str(BUILD_DEMO_APP_SCRIPT),
        "--skip-build",  # we handle that ourselves
    ]

    if skip_example_build:
        cmd.append("--skip-example-build")

    run(cmd, cwd=RERUN_DIR)


def collect_demo_examples() -> Iterable[Example]:
    web_demo_example_dir = SCRIPT_DIR_PATH.parent.parent / "web_demo" / "examples"
    assert web_demo_example_dir.exists(), "Web demos have not been built yet."

    manifest = json.loads((web_demo_example_dir / "manifest.json").read_text())

    for example in manifest:
        name = example["name"]
        rrd = web_demo_example_dir / f"{name}" / "data.rrd"
        assert rrd.exists(), f"Missing {rrd} for {name}"

        yield Example(
            name=name,
            title=example["title"],
            rrd=rrd,
            screenshot_url=example["thumbnail"]["url"],
        )


def collect_examples() -> Iterable[Example]:
    yield from collect_code_examples()
    yield from collect_demo_examples()


def render_index(examples: list[Example]) -> None:
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


def serve_files() -> None:
    def serve() -> None:
        print("\nServing examples at http://127.0.0.1:8080/\n")
        server = http.server.HTTPServer(
            server_address=("127.0.0.1", 8080),
            RequestHandlerClass=partial(
                http.server.SimpleHTTPRequestHandler,
                directory=BASE_PATH,
            ),
        )
        server.serve_forever()

    threading.Thread(target=serve, daemon=True).start()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--serve",
        action="store_true",
        help="Serve the app on this port after building [default: 8080]",
    )
    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK and web viewer Wasm.")
    parser.add_argument("--skip-example-build", action="store_true", help="Skip building the RRDs.")

    args = parser.parse_args()

    if not args.skip_build:
        build_python_sdk()
        build_wasm()

    if not args.skip_example_build:
        build_code_examples()
        build_demo_examples()

    examples = list(collect_examples())
    assert len(examples) > 0, "No examples found"

    render_index(examples)
    render_examples(examples)
    copy_static_assets(examples)
    copy_wasm(examples)

    if args.serve:
        serve_files()

        while True:
            try:
                print("Press enter to reload static files")
                input()
                render_examples(examples)
                copy_static_assets(examples)
                copy_wasm(examples)
            except KeyboardInterrupt:
                break


if __name__ == "__main__":
    main()
