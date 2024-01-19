from __future__ import annotations

import argparse
import math

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Simple benchmark for many individual entitites.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_benchmark_many_entities")

    for i in range(1000):
        f = i * 0.1
        rr.log("i" + str(i), rr.Points3D([math.sin(f), f, math.cos(f)]))


if __name__ == "__main__":
    main()
