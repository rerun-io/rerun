"""Logs a `Transform3D` archetype for roundtrip checks."""

#!/usr/bin/env python3

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "roundtrip_transform3d")

    rr.log_rect("placeholder", [0, 0, 4, 6])

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
