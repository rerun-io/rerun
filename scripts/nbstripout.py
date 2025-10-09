"""Simple Wrapper around nbstripout to strip notebooks in a directory."""

from __future__ import annotations

import argparse
import logging
import subprocess
from pathlib import Path

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Strip output from Jupyter notebooks.")
    parser.add_argument("directories", nargs="+", help="Directories to strip notebooks in")
    args, unknown_args = parser.parse_known_args()

    # Find all Jupyter notebooks in the directory
    notebook_dirs = [Path(d).rglob("*.ipynb") for d in args.directories]

    for notebook_dir in notebook_dirs:
        for notebook in notebook_dir:
            logging.debug(["nbstripout", str(notebook), *unknown_args])
            subprocess.run(["nbstripout", str(notebook), *unknown_args])
