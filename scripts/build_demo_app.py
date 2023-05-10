#!/usr/bin/env python3

"""Build `demo.rerun.io`."""

import os
import shutil
import subprocess
from typing import List

from jinja2 import Template

BASE_PATH = "web_demo"
SCRIPT_PATH = os.path.dirname(os.path.relpath(__file__))
EXAMPLES = [
    "api_demo",
    "car",
    "clock",
    "colmap",
    "deep_sdf",
    "dicom",
    "nyud",
    "plots",
    "raw_mesh",
    "text_logging",
]


class Example:
    def __init__(self, name: str):
        self.path = os.path.join("examples/python", name, "main.py")
        self.name = name
        self.source_url = f"https://github.com/rerun-io/rerun/tree/main/examples/python/{self.name}/main.py"

    def save(self) -> None:
        in_path = os.path.abspath(self.path)
        out_dir = f"{BASE_PATH}/examples/{self.name}"

        print(f"\nRunning {in_path}, outputting to {out_dir}")
        os.makedirs(out_dir, exist_ok=True)
        subprocess.run(
            [
                "python",
                in_path,
                "--num-frames=30",
                "--steps=200",
                f"--save={out_dir}/data.rrd",
            ],
            check=True,
        )

    def supports_save(self) -> bool:
        with open(self.path) as f:
            return "script_add_args" in f.read()


def clean() -> None:
    shutil.rmtree(BASE_PATH, ignore_errors=True)


def copy_static_assets() -> None:
    shutil.copytree(os.path.join(SCRIPT_PATH, "demo_assets/static"), os.path.join(BASE_PATH))


def build_and_copy_wasm() -> None:
    subprocess.run(["cargo", "r", "-p", "re_build_web_viewer", "--", "--release"])
    subprocess.run(["cargo", "r", "-p", "re_build_web_viewer", "--", "--debug"])

    files = ["re_viewer_bg.wasm", "re_viewer_debug_bg.wasm", "re_viewer.js", "re_viewer_debug.js"]
    for file in files:
        shutil.copyfile(
            os.path.join("web_viewer", file),
            os.path.join(BASE_PATH, file),
        )


def build_examples() -> List[Example]:
    examples: List[Example] = []
    for path in EXAMPLES:
        example = Example(path)
        if example.supports_save():
            example.save()
            examples.append(example)
    return examples


def render_examples(examples: List[Example]) -> None:
    template_path = os.path.join(SCRIPT_PATH, "demo_assets/templates/example.html")
    with open(template_path) as f:
        template = Template(f.read())

    for example in examples:
        index_path = f"{BASE_PATH}/examples/{example.name}/index.html"
        with open(index_path, "w") as f:
            f.write(template.render(example=example, examples=examples))


def main() -> None:
    clean()
    copy_static_assets()
    build_and_copy_wasm()
    examples = build_examples()
    render_examples(examples)


if __name__ == "__main__":
    main()
