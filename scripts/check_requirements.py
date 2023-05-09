#!/usr/bin/env python3

import os
from glob import glob


def main() -> None:
    print("checking `examples/python/requirements.txt`")
    with open("examples/python/requirements.txt") as f:
        requirements = set(f.read().strip().splitlines())

    missing = []
    for path in glob("examples/python/*/requirements.txt"):
        line = f"-r {os.path.relpath(path, 'examples/python')}"
        if line not in requirements:
            missing.append(line)

    if len(missing) != 0:
        print("`examples/python/requirements.txt` is missing the following requirements:")
        for line in missing:
            print(line)
        exit(1)

    print("`examples/python/requirements.txt` is not missing any requirements")


if __name__ == "__main__":
    main()
