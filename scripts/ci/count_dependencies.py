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
import re
import subprocess
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

    # An empty `RUSTFLAGS` disables `cfg(rerun_workspace_hack)`, so `re_workspace_hack` (which depends on every
    # third-party crate in the workspace) is not counted. It is a build-time optimization that users never get.
    custom_env = os.environ | {"RUSTFLAGS": ""}
    cmd = ["cargo", "tree", "--edges", "normal", "-p", crate, *flags.split()]
    print(f"Running command: {' '.join(cmd)}", file=sys.stderr, flush=True)
    tree = subprocess.run(cmd, env=custom_env, check=True, capture_output=True, text=True).stdout
    # Skip the root crate on the first line, and count every other `name vX.Y.Z` once.
    count = len(set(re.findall(r"\w+ v[0-9.]+", "\n".join(tree.splitlines()[1:]))))
    assert count > 0, f"Command failed. Maybe unknown crate? cmd: {cmd}"
    print(json.dumps([{"name": f"{crate} {flags}", "value": count, "unit": "crates"}]))


if __name__ == "__main__":
    main()
