#!/usr/bin/env python3

"""Build `demo.rerun.io`."""

import argparse
import http.server
import logging
import os
import shutil
import subprocess
import threading
from functools import partial
from typing import List

from jinja2 import Template


class Example:
    def __init__(self, name: str, title: str, description: str):
        self.path = os.path.join("examples/python", name, "main.py")
        self.name = name
        self.source_url = f"https://github.com/rerun-io/rerun/tree/main/examples/python/{self.name}/main.py"
        self.title = title

        segments = [f"<p>{segment}</p>" for segment in description.split("\n\n")]
        self.description = "".join(segments)

    def save(self) -> None:
        in_path = os.path.abspath(self.path)
        out_dir = f"{BASE_PATH}/examples/{self.name}"

        logging.info(f"Running {in_path}, outputting to {out_dir}")
        os.makedirs(out_dir, exist_ok=True)
        process = subprocess.run(
            [
                "python",
                in_path,
                "--num-frames=30",
                "--steps=200",
                f"--save={out_dir}/data.rrd",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )

        if process.returncode != 0:
            output = process.stdout.decode("utf-8").strip()
            logging.error(f"Running {in_path} failed:\n{output}")
            exit(process.returncode)

    def supports_save(self) -> bool:
        with open(self.path) as f:
            return "script_add_args" in f.read()


def copy_static_assets() -> None:
    src = os.path.join(SCRIPT_PATH, "demo_assets/static")
    dst = os.path.join(BASE_PATH)
    logging.info(f"Copying static assets from {src} to {dst}")
    shutil.copytree(src, dst, dirs_exist_ok=True)


def build_and_copy_wasm() -> None:
    logging.info("")
    subprocess.run(["cargo", "r", "-p", "re_build_web_viewer", "--", "--release"])
    subprocess.run(["cargo", "r", "-p", "re_build_web_viewer", "--", "--debug"])

    files = ["re_viewer_bg.wasm", "re_viewer_debug_bg.wasm", "re_viewer.js", "re_viewer_debug.js"]
    for file in files:
        shutil.copyfile(
            os.path.join("web_viewer", file),
            os.path.join(BASE_PATH, file),
        )


def collect_examples() -> List[Example]:
    examples = []
    for name in EXAMPLES.keys():
        example = Example(
            name,
            title=EXAMPLES[name]["title"],
            description=EXAMPLES[name]["description"],
        )
        if example.supports_save():
            examples.append(example)
    return examples


def save_examples_rrd(examples: List[Example]) -> None:
    logging.info("\nSaving examples as .rrd")

    for example in examples:
        example.save()


def render_examples(examples: List[Example]) -> None:
    logging.info("\nRendering examples")

    template_path = os.path.join(SCRIPT_PATH, "demo_assets/templates/example.html")
    with open(template_path) as f:
        template = Template(f.read())

    for example in examples:
        index_path = f"{BASE_PATH}/examples/{example.name}/index.html"
        logging.info(f"{example.name} -> {index_path}")
        with open(index_path, "w") as f:
            f.write(template.render(example=example, examples=examples))


def serve_files() -> None:
    def serve() -> None:
        logging.info("\nServing examples at http://localhost:8080/")
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
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(description="Build and/or serve `demo.rerun.io`")
    parser.add_argument(
        "--serve",
        action="store_true",
        help="Serve the app on this port after building [default: 8080]",
    )

    args, unknown = parser.parse_known_args()
    for arg in unknown:
        logging.warning(f"unknown arg: {arg}")

    copy_static_assets()
    build_and_copy_wasm()
    examples = collect_examples()
    save_examples_rrd(examples)
    render_examples(examples)

    if args.serve:
        serve_files()

        while True:
            try:
                logging.info("Press enter to reload static files")
                input()
                copy_static_assets()
                render_examples(examples)
            except KeyboardInterrupt:
                break


BASE_PATH = "web_demo"
SCRIPT_PATH = os.path.dirname(os.path.relpath(__file__))
# When adding examples, add their requirements to `requirements-demo.txt`
EXAMPLES = {
    "api_demo": {
        "title": "API Demo",
        "description": """
        This is a swiss-army-knife example showing the usage of most of the Rerun SDK APIs.
        The data logged is static and meaningless.
        """,
    },
    "car": {
        "title": "Car",
        "description": """
        A very simple 2D car is drawn using OpenCV, and a depth image is simulated and logged as a point cloud.
        """,
    },
    "clock": {
        "title": "Clock",
        "description": """
        An example visualizing an analog clock with hour, minute and seconds hands using Rerun Arrow3D primitives.
        """,
    },
    "colmap": {
        "title": "COLMAP",
        "description": """
        An example using Rerun to log and visualize the output of COLMAP's sparse reconstruction.

        <a href="https://colmap.github.io/index.html" target="_blank">COLMAP</a>
        is a general-purpose Structure-from-Motion (SfM)
        and Multi-View Stereo (MVS) pipeline with a graphical and command-line interface.

        In this example a short video clip has been processed offline by the COLMAP pipeline,
        and we use Rerun to visualize the individual camera frames, estimated camera poses,
        and resulting point clouds over time.
        """,
    },
    "deep_sdf": {
        "title": "Deep SDF",
        "description": """
        Generate Signed Distance Fields for arbitrary meshes using both traditional methods
        as well as the one described in the
        <a href="https://arxiv.org/abs/1901.05103" target="_blank">DeepSDF paper</a>,
        and visualize the results using the Rerun SDK.
        """,
    },
    "dicom": {
        "title": "Dicom",
        "description": """
        Example using a <a href="https://en.wikipedia.org/wiki/DICOM" target="_blank">DICOM</a> MRI scan.
        This demonstrates the flexible tensor slicing capabilities of the Rerun viewer.
        """,
    },
    "plots": {
        "title": "Plots",
        "description": """
        This example demonstrates how to log simple plots with the Rerun SDK.
        Charts can be created from 1-dimensional tensors, or from time-varying scalars.
        """,
    },
    "raw_mesh": {
        "title": "Raw Mesh",
        "description": """
        This example demonstrates how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups")
        and their transform hierarchy. Simple material properties are supported.
        """,
    },
    "text_logging": {
        "title": "Text Logging",
        "description": """
        This example demonstrates how to integrate python's native `logging` with the Rerun SDK.

        Rerun is able to act as a Python logging handler, and can show all your Python log messages
        in the viewer next to your other data.
        """,
    },
}

if __name__ == "__main__":
    main()
