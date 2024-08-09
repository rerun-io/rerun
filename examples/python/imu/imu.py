#!/usr/bin/env python3
"""Logging data from inertial measurement unit."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb


def main() -> None:
    parser = argparse.ArgumentParser(description="Example of logging IMU data using the `send_column` function.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "IMU")

    blueprint = rrb.Blueprint(
        rrb.Vertical(*(rrb.TimeSeriesView(origin=label) for label in ["Acc", "Gyro", "Mag"])),
        rrb.SelectionPanel(expanded=False),
        rrb.TimePanel(expanded=False),
        rrb.BlueprintPanel(expanded=False),
    )

    rr.send_blueprint(blueprint)

    labels = ["Acc/x", "Acc/y", "Acc/z", "Gyro/x", "Gyro/y", "Gyro/z", "Mag/x", "Mag/y", "Mag/z"]

    # Must download `nicla_fixed.csv` manually from https://www.kaggle.com/datasets/hkayan/industrial-robotic-arm-anomaly-detection/data
    arr = np.loadtxt("archive/nicla_fixed.csv", delimiter=",", dtype=float, skiprows=1)

    n = 100000
    for col_idx, label in enumerate(labels):
        column = arr[:n, col_idx]
        rr.send_columns(
            f"{label}",
            times=[rr.TimeSequenceBatch("step", np.arange(len(column)))],
            components=[rr.Scalar.indicator(), rr.components.ScalarBatch(column)],
        )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
