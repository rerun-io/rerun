#!/usr/bin/env python3

"""Logs a `Arrows3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr
import rerun.experimental as rr2


def main() -> None:
    origins = [[1, 2, 3], [10, 20, 30]]
    vectors = [[4, 5, 6], [40, 50, 60]]
    radii = np.array([0.1, 1.0], dtype=np.float32)
    colors = np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    )
    labels = ["hello", "friend"]
    class_ids = np.array([126, 127], dtype=np.uint64)
    instance_keys = np.array([66, 666], dtype=np.uint64)

    arrows3d = rr2.Arrows3D(
        vectors,
        origins=origins,
        radii=radii,
        colors=colors,
        labels=labels,
        class_ids=class_ids,
        instance_keys=instance_keys,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.rscript_setup(args, "rerun-example-roundtrip_arrows3d")

    rr2.log("arrows3d", arrows3d)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
