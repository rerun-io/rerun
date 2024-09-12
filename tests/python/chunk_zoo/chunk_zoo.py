"""
A collection of delightfully unique chunk specimens, for science!.

IMPORTANT: the viewer should be set with `RERUN_CHUNK_MAX_BYTES=0` to disable the compactor.

To add new specimens to the zoo, add a function whose name starts with "specimen_".
"""

from __future__ import annotations

import argparse
from typing import Sequence

import rerun as rr
import rerun.components as rrc


def times(t: int | Sequence[int], *args: int) -> list[rr.TimeSequenceColumn]:
    if isinstance(t, int):
        t = [t]
    else:
        t = list(t)
    if args:
        t.extend(args)
    return [rr.TimeSequenceColumn("frame", t)]


rr.init("rerun_example_chunk_zoo")
rr.connect()


def specimen_two_rows_span_two_chunks():
    """Two rows spanning two chunks."""

    rr.send_columns(
        "/rows_span_two_chunks",
        times(0, 2),
        [
            rrc.Position2DBatch([(0, 1), (2, 3)]),
        ],
    )
    rr.send_columns(
        "/rows_span_two_chunks",
        times(0, 2),
        [
            rrc.RadiusBatch([10, 11]),
        ],
    )


def specimen_two_rows_span_two_chunks_sparse():
    """Two rows spanning two chunks with partially matching timestamps (so sparse results)."""

    rr.send_columns(
        "/rows_span_two_chunks_sparse",
        times(0, 2, 3),
        [
            rrc.Position2DBatch([(0, 1), (2, 3), (4, 5)]),
        ],
    )
    rr.send_columns(
        "/rows_span_two_chunks_sparse",
        times(0, 2, 4),
        [
            rrc.RadiusBatch([10, 11, 12]),
        ],
    )


def specimen_archetype_with_clamp_join_semantics():
    """Single row of an archetype with clamp join semantics (Points2D)."""
    rr.send_columns(
        "/archetype_with_clamp_join_semantics",
        times(0),
        [
            rrc.Position2DBatch([(i, i) for i in range(10)]).partition([10]),
            rrc.RadiusBatch([2]),
            rr.Points2D.indicator(),
        ],
    )


def specimen_archetype_with_clamp_join_semantics_two_chunks():
    """Single row of an archetype with clamp join semantics (Points2D), across two chunks."""
    rr.send_columns(
        "/archetype_with_clamp_join_semantics_two_batches",
        times(0),
        [
            rrc.Position2DBatch([(i, i) for i in range(10)]).partition([10]),
        ],
    )

    rr.send_columns(
        "/archetype_with_clamp_join_semantics_two_batches",
        times(0),
        [
            rrc.RadiusBatch([2]),
            rr.Points2D.indicator(),
        ],
    )


def specimen_archetype_without_clamp_join_semantics():
    """Single row of an archetype without clamp join semantics (Mesh3D)."""
    rr.send_columns(
        "/archetype_without_clamp_join_semantics",
        times(0),
        [
            rrc.Position3DBatch([(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)]).partition([4]),
            rrc.TriangleIndicesBatch([(0, 1, 2), (0, 2, 3)]).partition([2]),
            rrc.ColorBatch([(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0)]).partition([4]),
            rr.Mesh3D.indicator(),
        ],
    )


# TODO(ab): add variants (unordered, overlapping timestamps, etc.)
def specimen_scalars_interlaced_in_two_chunks():
    """Scalar column stored in two chunks, with interlaced timestamps."""
    rr.send_columns(
        "/scalars_interlaced_in_two_chunks",
        times(0, 2, 5, 6, 8),
        [
            rrc.ScalarBatch([0, 2, 5, 6, 8]),
        ],
    )
    rr.send_columns(
        "/scalars_interlaced_in_two_chunks",
        times(1, 3, 7),
        [
            rrc.ScalarBatch([1, 3, 7]),
        ],
    )


def main():
    parser = argparse.ArgumentParser(
        description="Logs a bunch of chunks of various typologies. Use `RERUN_CHUNK_MAX_BYTES=0`!"
    )
    parser.add_argument("--filter", type=str, help="Only run specimens whose name contains this substring")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_chunk_zoo", default_blueprint=rr.blueprint.TextDocumentView(origin="/info"))

    # Round up the specimens
    specimens = [
        globals()[name]
        for name in globals()
        if name.startswith("specimen_") and callable(globals()[name]) and (not args.filter or args.filter in name)
    ]

    # Create an inventory of all the specimens
    def strip_prefix(s: str) -> str:
        if s.startswith("specimen_"):
            return s[len("specimen_") :]

    specimen_list = "\n".join([f"| {strip_prefix(s.__name__)} | {s.__doc__} |" for s in specimens])
    markdown = (
        "# Chunk Zoo\n\n"
        "This recording contains a variety of chunks of various typologies, for testing purposes.\n\n"
        "**IMPORTANT**: The viewer should be set with `RERUN_CHUNK_MAX_BYTES=0` to disable the compactor.\n\n"
        "### Specimens\n\n"
        f"| **Item** | **Description** |\n| --- | --- |\n{specimen_list}"
    )
    rr.log("info", rr.TextDocument(text=markdown, media_type="text/markdown"), static=True)

    # Set the specimens loose
    for specimen in specimens:
        specimen()


if __name__ == "__main__":
    main()
