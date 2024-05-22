from __future__ import annotations

import argparse
import sys
from pathlib import Path
import importlib

import pyarrow as pa

import rerun as rr


class TestTriggerIndicatorComponentBatch:
    """Marker component for in-viewer test."""

    data: pa.Array

    def __init__(self, test_name: str) -> None:
        self.data = pa.array([test_name], type=pa.string())

    def component_name(self) -> str:
        return "TestTriggerIndicator"

    def as_arrow_array(self) -> pa.Array:
        return self.data


def log_tests(args: argparse.Namespace) -> None:
    modules = [m for m in (Path(__file__).parent / "test_cases").glob("*.py")]

    for module in modules:
        test_name = module.stem

        m = importlib.import_module(f"test_cases.{module.stem}", package="test_cases")
        if hasattr(m, "log_test_data"):
            rr.script_setup(args, f"rerun_example_{test_name}_in_viewer_test")
            rr.log_components("test_trigger", [TestTriggerIndicatorComponentBatch(test_name)])

            m.log_test_data()
        else:
            print(f"Module {module.name} does not have a log_test_data function.", file=sys.stderr)


def main() -> None:
    parser = argparse.ArgumentParser(description="In-Viewer Tests")
    rr.script_add_args(parser)
    args = parser.parse_args()

    log_tests(args)


if __name__ == "__main__":
    main()
