"""Simple Wrapper around nbstripout to strip notebooks in a directory."""

from __future__ import annotations

import argparse
import logging
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from rich.progress import track


def _strip_notebook(notebook_path: Path, extra_args: list[str]) -> None:
    """Strip output from a single Jupyter notebook."""
    logging.debug(["nbstripout", str(notebook_path), *extra_args])
    subprocess.run(["nbstripout", str(notebook_path), *extra_args], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Strip output from Jupyter notebooks.")
    parser.add_argument("--directories", nargs="+", help="Directories to strip notebooks in")
    args, unknown_args = parser.parse_known_args()
    # Strip separator if present
    if "--" in unknown_args:
        unknown_args.remove("--")

    # Find all Jupyter notebooks in the directories
    notebook_dirs = [Path(d).rglob("*.ipynb") for d in args.directories]

    futures = []
    cpu_count = 4
    if (total_cores := os.cpu_count()) is not None:
        cpu_count = total_cores - 2
    print(f"Using {cpu_count} threads to strip notebooks.")
    with ThreadPoolExecutor(max_workers=cpu_count) as executor:
        for notebook_dir in notebook_dirs:
            # print(f"Stripping notebooks in directory: {list(notebook_dir)}")
            for notebook in list(notebook_dir):
                futures.append(executor.submit(_strip_notebook, notebook, unknown_args))
    for future in track(futures, description="Stripping notebooks..."):
        future.result()
