#!/usr/bin/env python3

"""
Count the total number of dependencies of a file (recursively).

This produces the format for use in <https://github.com/benchmark-action/github-action-benchmark>.

Use the script:
    python3 scripts/ci/count_dependencies.py -p rerun --all-features
    python3 scripts/ci/count_dependencies.py -p rerun --no-default-features

Unfortunately, this script under-counts compared to what `cargo build` outputs.
There is also `cargo deps-list`, which also under-counts.
For instance:

* `scripts/ci/count_dependencies.py -p re_sdk --no-default-features`  => 118
* `cargo deps-list -p re_sdk --no-default-features`                   => 165
* `cargo check -p re_sdk --no-default-features`                       => 213

So this script is more of a heurristic than an exact count.
"""

from __future__ import annotations

import argparse
import json
import os
import sys


def main() -> None:
    parser = argparse.ArgumentParser(description="Count crate dependencies")

    parser.add_argument("-p", required=True, type=str, help="Crate name")
    parser.add_argument("--all-features", default=False, action="store_true", help="Use all features")
    parser.add_argument("--no-default-features", default=False, action="store_true", help="Use no default features")
    parser.add_argument("-F", "--features", default="", type=str, help="Additional features to enable")

    args = parser.parse_args()

    crate = args.p
    if args.all_features:
        flags = "--all-features"
    elif args.no_default_features:
        flags = "--no-default-features"
    else:
        flags = ""
    if args.features:
        flags += f" --features {args.features}"

    cmd = f'cargo tree --edges normal -p {crate} {flags} | tail -n +2 | grep -E "\\w+ v[0-9.]+" -o | sort -u | wc -l'
    print(f"Running command: {cmd}", file=sys.stderr, flush=True)
    count = int(os.popen(cmd).read().strip())
    assert count > 0, f"Command failed. Maybe unknown crate? cmd: {cmd}"
    print(json.dumps([{"name": f"{crate} {flags}", "value": count, "unit": "crates"}]))


if __name__ == "__main__":
    main()
