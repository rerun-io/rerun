#!/usr/bin/env python3

"""
Execute every example notebook headlessly and fail if any of them raises.

No browser is involved, so the viewer widget never loads: this catches crashes in the
Python side of the notebook integration, not rendering problems.
"""

from __future__ import annotations

import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

NOTEBOOK_DIR = Path(__file__).resolve().parents[2] / "examples" / "notebook"

# Shortens the slowest notebook's training loop from about a minute to a few seconds.
NEURAL_FIELD_PATCH = ("num_iterations = 3000", "num_iterations = 30")

# Only catches hangs: the slowest notebook takes about 8 s on a laptop.
TIMEOUT_SECS = 600


def run(notebook: Path) -> subprocess.CompletedProcess[str]:
    if notebook.suffix == ".py":
        # `marimo export` exits non-zero when a cell fails.
        cmd = [sys.executable, "-m", "marimo", "export", "html", "--force", f"--output={os.devnull}", notebook.name]
        source = None
    else:
        # Piping the notebook through stdin lets us patch it while the kernel still runs in its directory.
        cmd = [sys.executable, "-m", "jupyter", "nbconvert", "--stdin", "--stdout", "--to=notebook", "--execute"]
        source = notebook.read_text(encoding="utf-8")
        if notebook.name == "neural_field_2d.ipynb":
            old, new = NEURAL_FIELD_PATCH
            if old not in source:
                raise ValueError(f"Patch no longer matches: {old!r}\nNotebook: {notebook}")
            source = source.replace(old, new)

    return subprocess.run(
        cmd,
        cwd=notebook.parent,
        input=source,
        capture_output=True,
        text=True,
        timeout=TIMEOUT_SECS,
        check=False,
    )


def main() -> None:
    notebooks = sorted(NOTEBOOK_DIR.glob("*/*.ipynb")) + sorted(NOTEBOOK_DIR.glob("*/*_marimo.py"))

    # Each notebook starts its own kernel; 4 at once keeps a CI agent from being oversubscribed.
    with ThreadPoolExecutor(max_workers=4) as pool:
        failed = False
        for notebook, result in zip(notebooks, pool.map(run, notebooks), strict=True):
            ok = result.returncode == 0
            print(f"{'ok  ' if ok else 'FAIL'} {notebook.relative_to(NOTEBOOK_DIR)}", flush=True)
            if not ok:
                print(result.stderr + result.stdout, flush=True)
                failed = True

    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
