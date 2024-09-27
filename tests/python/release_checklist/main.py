#!/usr/bin/env python3
from __future__ import annotations

import argparse
from os.path import basename, dirname, isfile, join

import rerun as rr


def log_checks(args: argparse.Namespace) -> None:
    import glob
    import importlib

    modules = glob.glob(join(dirname(__file__), "*.py"))
    modules = [basename(f)[:-3] for f in modules if isfile(f) and basename(f).startswith("check_")]

    for module in modules:
        if args.skip_checks_with_assets and "check_video" in module:
            continue

        m = importlib.import_module(module)
        m.run(args)


def log_readme() -> None:
    with open(join(dirname(__file__), "README.md"), encoding="utf8") as f:
        rr.log("readme", rr.TextDocument(f.read(), media_type=rr.MediaType.MARKDOWN), static=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Interactive release checklist")
    parser.add_argument(
        "--skip-checks-with-assets",
        action="store_true",
        help="Skip checks that require downloading test assets",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    log_checks(args)

    # Log instructions last so that's what people see first.
    rr.script_setup(args, "instructions")
    log_readme()


if __name__ == "__main__":
    main()
