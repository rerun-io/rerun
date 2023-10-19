#!/usr/bin/env python3

"""Checks that no links with `#new` are present in any `.md` files."""

from __future__ import annotations

import re
from concurrent.futures import ThreadPoolExecutor
from glob import glob

NEW_LINK_HASH = "#new"


def check_file(file_path: str) -> list[str] | None:
    links = []
    with open(file_path) as f:
        content = f.read()
        for m in re.finditer("(https://.*)#new", content):
            line_number = 1 + content[: m.start()].count("\n")
            links.append(f"{m.group(0)} ({file_path}:{line_number})")

    if len(links) > 0:
        return links
    else:
        return None


def main() -> None:
    with ThreadPoolExecutor() as e:
        bad_files = [v for v in e.map(check_file, glob("**/*.md", recursive=True)) if v is not None]
        if len(bad_files) > 0:
            print("The following `#new` URLs were found:")
            for file in bad_files:
                for link in file:
                    print(link)
            exit(1)


if __name__ == "__main__":
    main()
