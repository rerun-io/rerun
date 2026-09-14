#!/usr/bin/env -S uv run --quiet --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "pyarrow"]
# ///
"""Check that joint values in a parquet column stay inside a URDF's joint limits.

Reads one list-valued column (one row per frame, one entry per joint), and for each
joint prints the value range, the URDF limit, and which unit/sign/wrap/offset
corrections keep the values in range.

Use it as a sanity check before trusting a pose: the mapping from a dataset's joint
values onto a URDF's joints is not in the data, and the limits are the cheapest
mechanical check on it.

Limits narrow the candidates, they do not pick one. A joint with several surviving
candidates is ambiguous, and must be settled against a known pose or the robot's
geometry. A joint with none means the column is not that joint at all — check the
name ordering.

The URDF is parsed with the stdlib rather than `rerun.urdf.UrdfTree` so this runs
against any checkout without a built SDK; `UrdfTree.joints()` is the API to use from
inside a pipeline.
"""

from __future__ import annotations

import argparse
import math
import sys
import xml.etree.ElementTree as ET  # noqa: S405 — the URDF is a local file the caller named
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq

# Corrections are applied in the source unit (degrees or radians), before conversion.
# `%360` is the wrap that shows up whenever a servo reports a continuous angle.
CANDIDATES: dict[str, object] = {
    "identity": lambda x, full: x,
    "negate": lambda x, full: -x,
    "%360": lambda x, full: x % full,
    "negate,%360": lambda x, full: (-x) % full,
    "+90": lambda x, full: x + full / 4,
    "-90": lambda x, full: x - full / 4,
    "+180": lambda x, full: x + full / 2,
    "-180": lambda x, full: x - full / 2,
    "negate+90": lambda x, full: -x + full / 4,
    "negate-90": lambda x, full: -x - full / 4,
    "negate+180": lambda x, full: -x + full / 2,
}


@dataclass
class Joint:
    """One non-fixed URDF joint and its limits, in the source unit."""

    name: str
    joint_type: str
    lower: float
    upper: float


def read_joints(urdf_path: Path, degrees: bool) -> list[Joint]:
    """Parse the URDF's non-fixed joints, converting limits into the source unit."""
    scale = math.degrees(1.0) if degrees else 1.0
    joints = []
    for element in ET.parse(urdf_path).getroot().findall("joint"):  # noqa: S314
        joint_type = element.get("type", "")
        if joint_type == "fixed":
            continue
        limit = element.find("limit")
        lower = float(limit.get("lower", "-inf")) if limit is not None else -math.inf
        upper = float(limit.get("upper", "inf")) if limit is not None else math.inf
        name = element.get("name")
        if name is None:
            sys.exit(f"URDF has a joint without a name\nFile path: {urdf_path}")
        joints.append(Joint(name, joint_type, lower * scale, upper * scale))
    return joints


def read_values(paths: list[Path], column: str) -> np.ndarray:
    """Read one list-valued column from every parquet file into a (rows, joints) array."""
    chunks = []
    for path in paths:
        table = pq.read_table(path, columns=[column])
        if table.num_rows == 0:
            continue
        chunks.append(np.asarray(table.column(column).to_pylist(), dtype=np.float64))
    if not chunks:
        sys.exit(f"Column {column!r} yielded no rows\nFile path: {paths[0]}")
    values = np.concatenate(chunks)
    if values.ndim != 2:
        sys.exit(f"Column {column!r} is not list-valued (got shape {values.shape})")
    return values


def overshoot(values: np.ndarray, joint: Joint) -> tuple[float, float]:
    """Worst excursion outside the limits, and the fraction of rows outside them."""
    below = np.maximum(joint.lower - values, 0.0)
    above = np.maximum(values - joint.upper, 0.0)
    worst = np.maximum(below, above)
    return float(worst.max()), float((worst > 0.0).mean())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("urdf", type=Path)
    parser.add_argument("parquet", type=Path, nargs="+", help="one or more parquet files (e.g. every episode)")
    parser.add_argument("--column", required=True, help="list-valued column holding the joint values per row")
    parser.add_argument("--degrees", action="store_true", help="source values are degrees (URDF limits are radians)")
    parser.add_argument("--joints", help="comma-separated joint names in column order (default: URDF order)")
    parser.add_argument(
        "--slack",
        type=float,
        default=2.0,
        help="tolerated overshoot in the source unit; calibration slack, not a mapping bug (default: 2.0)",
    )
    args = parser.parse_args()

    unit = "deg" if args.degrees else "rad"
    full = 360.0 if args.degrees else 2.0 * math.pi

    urdf_joints = {joint.name: joint for joint in read_joints(args.urdf, args.degrees)}
    values = read_values(args.parquet, args.column)

    if args.joints:
        names = [name.strip() for name in args.joints.split(",")]
    else:
        names = list(urdf_joints)

    if len(names) != values.shape[1]:
        sys.exit(
            f"Joint count mismatch: {len(names)} names but the column has {values.shape[1]} values per row.\n"
            "Pass --joints with the names in column order (a gripper is often two URDF joints)."
        )
    missing = [name for name in names if name not in urdf_joints]
    if missing:
        sys.exit(f"Not URDF joints: {', '.join(missing)}\nFile path: {args.urdf}")

    print(f"{values.shape[0]} rows, values in {unit}, slack {args.slack} {unit}\n")
    ambiguous = []
    for index, name in enumerate(names):
        joint = urdf_joints[name]
        column = values[:, index]
        print(
            f"{name}  ({joint.joint_type})  "
            f"raw {column.min():.1f}…{column.max():.1f} {unit}  "
            f"limit {joint.lower:.1f}…{joint.upper:.1f} {unit}"
        )
        fits = []
        for label, correct in CANDIDATES.items():
            worst, fraction = overshoot(correct(column, full), joint)
            if worst == 0.0:
                print(f"    fits      {label:12s}")
                fits.append(label)
            elif worst <= args.slack:
                print(f"    fits*     {label:12s} over by {worst:.1f} {unit} on {fraction:.1%} of rows — clamp=True")
                fits.append(label)
        if not fits:
            print("    NONE fit — wrong joint for this column, or the name order is off")
        elif len(fits) > 1:
            ambiguous.append((name, fits))
        print()

    for name, fits in ambiguous:
        print(f"AMBIGUOUS {name}: {', '.join(fits)} — settle against a known pose or the robot's geometry")


if __name__ == "__main__":
    main()
