#!/usr/bin/env python3

"""Logs an `AnnotationContext` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr
from rerun.datatypes import ClassDescription


def main() -> None:
    annotation_context = rr.AnnotationContext(
        [
            (1, "hello"),
            ClassDescription(
                info=(2, "world", [3, 4, 5]),
                keypoint_annotations=[(17, "head"), (42, "shoulders")],
                keypoint_connections=[(1, 2), (3, 4)],
            ),
        ]
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_annotation_context")

    rr.log("annotation_context", annotation_context)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
