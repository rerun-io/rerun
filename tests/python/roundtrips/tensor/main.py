#!/usr/bin/env python3

"""Logs a `Tensor` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr
import rerun.experimental as rr2


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_tensor")

    tensor = np.array(np.arange(0, 360), dtype=np.int32).reshape((3, 4, 5, 6))
    tensor = rr2.dt.TensorData(array=tensor, id=np.arange(10, 26))

    rr2.log("tensor", rr2.Tensor(tensor))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
