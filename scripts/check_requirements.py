#!/usr/bin/env python3

import os
from glob import glob


def main() -> None:
    failed = False

    print("Checking `examples/python/requirements.txt`...")
    with open("examples/python/requirements.txt") as f:
        lines = f.read().strip().splitlines()
        sorted_lines = lines.copy()
        sorted_lines.sort()
        requirements = set(lines)

    missing = []
    for path in glob("examples/python/*/requirements.txt"):
        line = f"-r {os.path.relpath(path, 'examples/python')}"
        if line not in requirements:
            missing.append(line)

    if len(missing) != 0:
        print("\n`examples/python/requirements.txt` is missing the following requirements:")
        for line in missing:
            print(line)
        failed = True

    if lines != sorted_lines:
        print("\n`examples/python/requirements.txt` is not correctly sorted.")
        failed = True

    if failed:
        print("\nHere is what `examples/python/requirements.txt` should contain:")
        expected = glob("examples/python/*/requirements.txt")
        expected.sort()
        for path in expected:
            print(f"-r {os.path.relpath(path, 'examples/python')}")
        exit(1)
    else:
        print("All clear.")


if __name__ == "__main__":
    main()
