#!/usr/bin/env python3

"""Logs an `AnnotationContext` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd


def main() -> None:
    annotation_context = rr2.AnnotationContext(
        [
            (1, "hello"),
            rrd.ClassDescription(
                info=(2, "world", [3, 4, 5]),
                keypoint_annotations=[(17, "head"), (42, "shoulders")],
                keypoint_connections=[(1, 2), (3, 4)],
            ),
        ]
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.rscript_setup(args, "rerun-example-roundtrip_annotation_context")

    rr2.log("annotation_context", annotation_context)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
