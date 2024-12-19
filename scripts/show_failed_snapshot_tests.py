from __future__ import annotations

import argparse
from pathlib import Path
from typing import Iterator

import numpy as np
import PIL.Image as Image

import rerun as rr
import rerun.blueprint as rrb


def find_failed_snapshot_tests() -> Iterator[tuple[Path, Path, Path]]:
    crates_dir = Path(__file__).parent.parent / "crates"

    for diff_path in crates_dir.rglob("**/*.diff.png"):

        original_path = diff_path.parent / diff_path.name.replace(".diff.png", ".png")
        new_path = diff_path.parent / diff_path.name.replace(".diff.png", ".new.png")

        if original_path.exists() and new_path.exists():
            yield original_path, new_path, diff_path


def log_failed_snapshot_tests(original_path: Path, new_path: Path, diff_path: Path) -> None:
    recording = rr.new_recording(f"rerun_example_failed_{original_path.stem}")

    with recording:
        rr.connect_tcp(default_blueprint=rrb.Blueprint(
            rrb.Tabs(
                rrb.Horizontal(
                    rrb.Spatial2DView(origin="original", name="Original"),
                    rrb.Spatial2DView(origin="new", name="New"),
                    rrb.Spatial2DView(origin="diff", name="Diff"),
                    name="Side-by-side"
                ),
                rrb.Tabs(
                    rrb.Spatial2DView(origin="original", name="Original"),
                    rrb.Spatial2DView(origin="new", name="New"),
                    rrb.Spatial2DView(origin="diff", name="Diff"),
                    name="Overlay (tab)"
                ),
                rrb.Spatial2DView(contents=["/original", "/new"], name="Overlay (opacity)", overrides={
                    "/new": [rr.components.Opacity(0.5)],
                }),
            ),
            rrb.TimePanel(expanded=False),
        ))

        rr.log("original", rr.Image(np.array(Image.open(original_path))), static=True)
        rr.log("new", rr.Image(np.array(Image.open(new_path))), static=True)
        rr.log("diff", rr.Image(np.array(Image.open(diff_path))), static=True)


def main():
    parser = argparse.ArgumentParser(description="Logs all failed snapshot tests for comparison in rerun")
    _ = parser.parse_args()

    for original_path, new_path, diff_path in find_failed_snapshot_tests():
        log_failed_snapshot_tests(original_path, new_path, diff_path)


if __name__ == '__main__':
    main()
