#!/usr/bin/env python3
"""
A single entry point to run the Rerun any_scalar_example demos.

You can run either the robotics demo or the market demo:
    python -m any_scalar_example --demo robotics
    python -m any_scalar_example --demo market
"""

from __future__ import annotations

import argparse

import rerun as rr
from any_scalar import market_demo, robotics_demo


def main() -> None:
    parser = argparse.ArgumentParser(description="Rerun Any Scalar Example")
    parser.add_argument(
        "--demo",
        type=str,
        default="robotics",
        choices=["robotics", "market"],
        help="Which demo to run (default: robotics)",
    )

    # Add standard Rerun arguments
    rr.script_add_args(parser)
    args = parser.parse_args()

    # Initialize Rerun SDK
    rr.script_setup(args, "rerun_example_any_scalar")

    if args.demo == "robotics":
        robotics_demo.run_robotics_simulation()
        rr.send_blueprint(robotics_demo.generate_blueprint())
    elif args.demo == "market":
        market_demo.run_market_demo()
        rr.send_blueprint(market_demo.generate_blueprint())

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
