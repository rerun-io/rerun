#!/usr/bin/env python3

"""Build `.rrd` files for all examples for deployment to app.rerun.io."""

import os
import shutil
import subprocess
from glob import glob
from typing import List

from jinja2 import Template


class Example:
    def __init__(self, path: str):
        self.path = path
        self.name = os.path.basename(os.path.dirname(path))
        self.source_url = f"https://github.com/rerun-io/rerun/tree/main/examples/python/{self.name}/main.py"

    def save(self) -> None:
        in_path = os.path.abspath(self.path)
        out_dir = f"./web_viewer/examples/{self.name}"

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


def render_app_index(example: Example, all_examples: List[Example]) -> str:
    template_path = os.path.join(os.path.dirname(os.path.relpath(__file__)), "templates/app_index.html")
    with open(template_path) as f:
        template = Template(f.read())

    return template.render(example=example, examples=all_examples)


def main() -> None:
    assert os.path.exists("./web_viewer")

    shutil.rmtree("./web_viewer/examples", ignore_errors=True)

    # TODO(jprochazk): get these to work too
    filter = ["ros", "opencv_canny", "stable_diffusion", "segment_anything"]

    examples: List[Example] = []
    # run examples to produce `.rrd` files
    for path in glob("examples/python/**/main.py"):
        example = Example(path)
        if example.name not in filter and example.supports_save():
            example.save()
            examples.append(example)

    # render templates
    for example in examples:
        index_path = f"./web_viewer/examples/{example.name}/index.html"
        with open(index_path, "w") as f:
            f.write(render_app_index(example, examples))


if __name__ == "__main__":
    main()
