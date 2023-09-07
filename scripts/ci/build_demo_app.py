#!/usr/bin/env python3

"""Build `demo.rerun.io`."""
from __future__ import annotations

import argparse
import html.parser
import http.server
import json
import logging
import os
import shutil
import subprocess
import threading
from functools import partial
from io import BytesIO
from pathlib import Path
from typing import Any

import frontmatter
import requests
from jinja2 import Template
from PIL import Image


def measure_thumbnail(url: str) -> Any:
    """Downloads `url` and returns its width and height."""
    response = requests.get(url)
    response.raise_for_status()
    image = Image.open(BytesIO(response.content))
    return image.size


# https://stackoverflow.com/a/7778368
class HTMLTextExtractor(html.parser.HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.result: list[Any] = []

    def handle_data(self, d: Any) -> None:
        self.result.append(d)

    def get_text(self) -> str:
        return "".join(self.result)


def extract_text_from_html(html: str) -> str:
    """
    Strips tags and unescapes entities from `html`.

    This is not a sanitizer, it should not be used on untrusted input.
    """
    extractor = HTMLTextExtractor()
    extractor.feed(html)
    return extractor.get_text()


def run(
    args: list[str], *, env: dict[str, str] | None = None, timeout: int | None = None, cwd: str | None = None
) -> None:
    print(f"> {subprocess.list2cmdline(args)}")
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout, check=False, capture_output=True, text=True)
    assert (
        result.returncode == 0
    ), f"{subprocess.list2cmdline(args)} failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}"


class Example:
    def __init__(
        self,
        name: str,
        commit: str,
        build_args: list[str],
    ):
        readme_path = Path("examples/python", name, "README.md")
        if readme_path.exists():
            readme = frontmatter.loads(readme_path.read_text())
        else:
            readme = frontmatter.Post(content="")

        thumbnail_url = readme.get("thumbnail")
        if thumbnail_url:
            width, height = measure_thumbnail(thumbnail_url)
            thumbnail = {
                "url": thumbnail_url,
                "width": width,
                "height": height,
            }
        else:
            thumbnail = None

        self.path = os.path.join("examples/python", name, "main.py")
        self.name = name
        self.title = readme.get("title")
        self.description = readme.get("description")
        self.summary = readme.get("summary")
        self.tags = readme.get("tags", [])
        self.demo_url = f"https://demo.rerun.io/commit/{commit}/examples/{name}/"
        self.rrd_url = f"https://demo.rerun.io/commit/{commit}/examples/{name}/data.rrd"
        self.source_url = f"https://github.com/rerun-io/rerun/tree/{commit}/examples/python/{name}/main.py"
        self.thumbnail = thumbnail
        self.build_args = build_args
        self.description_html = "".join([f"<p>{segment}</p>" for segment in self.description.split("\n\n")])

    def save(self) -> None:
        in_path = os.path.abspath(self.path)
        out_dir = f"{BASE_PATH}/examples/{self.name}"

        os.makedirs(out_dir, exist_ok=True)
        rrd_path = os.path.join(out_dir, "data.rrd")
        logging.info(f"Running {self.name}, outputting to {rrd_path}")

        args = [
            "python3",
            in_path,
            f"--save={rrd_path}",
        ]

        # Configure flushing so that:
        # * the resulting file size is deterministic
        # * the file is chunked into small batches for better streaming
        env = {**os.environ, "RERUN_FLUSH_TICK_SECS": "1000000000", "RERUN_FLUSH_NUM_BYTES": str(128 * 1024)}
        run(args + self.build_args, env=env)
        print(f"{rrd_path}: {os.path.getsize(rrd_path) / (1024 * 1024):.1f} MiB")

    def supports_save(self) -> bool:
        with open(self.path) as f:
            return "script_add_args" in f.read()


def copy_static_assets(examples: list[Example]) -> None:
    # copy root
    src = os.path.join(SCRIPT_PATH, "demo_assets/static")
    dst = BASE_PATH
    logging.info(f"\nCopying static assets from {src} to {dst}")
    shutil.copytree(src, dst, dirs_exist_ok=True)

    # copy examples
    for example in examples:
        src = os.path.join(SCRIPT_PATH, "demo_assets/static")
        dst = os.path.join(BASE_PATH, f"examples/{example.name}")
        shutil.copytree(
            src,
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
    logging.info("")
    run(["cargo", "r", "-p", "re_build_web_viewer", "--", "--release"])


def copy_wasm(examples: list[Example]) -> None:
    files = ["re_viewer_bg.wasm", "re_viewer.js"]
    for example in examples:
        for file in files:
            shutil.copyfile(
                os.path.join("web_viewer", file),
                os.path.join(BASE_PATH, f"examples/{example.name}", file),
            )


def collect_examples() -> list[Example]:
    commit = os.environ.get("COMMIT_HASH") or "main"
    logging.info(f"Commit hash: {commit}")
    examples = []
    for name in EXAMPLES.keys():
        example = Example(
            name,
            commit=commit,
            build_args=EXAMPLES[name]["build_args"],
        )
        assert example.supports_save(), f'Example "{name}" does not support saving'
        examples.append(example)

    return examples


def save_examples_rrd(examples: list[Example]) -> None:
    logging.info("\nSaving examples as .rrd…")

    print("")
    for example in examples:
        example.save()
        print("")


def render_examples(examples: list[Example]) -> None:
    logging.info("Rendering examples")

    template_path = os.path.join(SCRIPT_PATH, "demo_assets/templates/example.html")
    with open(template_path) as f:
        template = Template(f.read())

    for example in examples:
        index_path = f"{BASE_PATH}/examples/{example.name}/index.html"
        logging.info(f"{example.name} -> {index_path}")
        with open(index_path, "w") as f:
            f.write(template.render(example=example, examples=examples))


def render_manifest(examples: list[Example]) -> None:
    logging.info("Rendering index")

    index = []
    for example in examples:
        index.append(
            {
                "name": example.name,
                "title": example.title,
                "description": example.description,
                "tags": example.tags,
                "demo_url": example.demo_url,
                "rrd_url": example.rrd_url,
                "thumbnail": example.thumbnail,
            }
        )

    index_dir = Path(BASE_PATH) / "examples"
    index_dir.mkdir(parents=True, exist_ok=True)
    index_path = index_dir / "manifest.json"
    index_path.write_text(json.dumps(index))


def serve_files() -> None:
    def serve() -> None:
        logging.info("\nServing examples at http://0.0.0.0:8080/")
        server = http.server.HTTPServer(
            server_address=("0.0.0.0", 8080),
            RequestHandlerClass=partial(
                http.server.SimpleHTTPRequestHandler,
                directory=BASE_PATH,
            ),
        )
        server.serve_forever()

    threading.Thread(target=serve, daemon=True).start()


def main() -> None:
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(description="Build and/or serve `demo.rerun.io`")
    parser.add_argument(
        "--serve",
        action="store_true",
        help="Serve the app on this port after building [default: 8080]",
    )

    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK and web viewer Wasm.")
    parser.add_argument("--skip-examples", action="store_true", help="Skip running the examples.")

    args = parser.parse_args()

    if not args.skip_build:
        build_python_sdk()
        build_wasm()

    examples = collect_examples()
    assert len(examples) > 0, "No examples found"

    if not args.skip_examples:
        shutil.rmtree(f"{BASE_PATH}/examples", ignore_errors=True)
        render_manifest(examples)
        save_examples_rrd(examples)

    render_examples(examples)
    copy_static_assets(examples)
    copy_wasm(examples)

    if args.serve:
        serve_files()

        while True:
            try:
                logging.info("Press enter to reload static files")
                input()
                render_examples(examples)
                copy_static_assets(examples)
                copy_wasm(examples)
            except KeyboardInterrupt:
                break


BASE_PATH = "web_demo"
SCRIPT_PATH = os.path.dirname(os.path.relpath(__file__))
# When adding examples, add their requirements to `requirements-web-demo.txt`
EXAMPLES: dict[str, Any] = {
    "arkit_scenes": {
        "build_args": [],
    },
    "structure_from_motion": {
        "build_args": ["--dataset=colmap_fiat", "--resize=800x600"],
    },
    "dicom_mri": {
        "build_args": [],
    },
    "human_pose_tracking": {
        "build_args": [],
    },
    "plots": {
        "build_args": [],
    },
    "detect_and_track_objects": {
        "build_args": [],
    },
    "dna": {
        "build_args": [],
    },
}

if __name__ == "__main__":
    main()
