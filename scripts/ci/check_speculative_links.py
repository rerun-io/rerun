#!/usr/bin/env python3

"""Checks that no links with a speculative link marker are present in any `.md` files."""

from __future__ import annotations

import re
import sys
import urllib.parse
from concurrent.futures import ThreadPoolExecutor
from glob import glob

NEW_LINK_MARKER = "speculative-link"


def check_file(file_path: str) -> list[str] | None:
    links = []
    with open(file_path, encoding="utf8") as f:
        content = f.read()
        for m in re.finditer(f"(https://.*){NEW_LINK_MARKER}", content):
            link = m.group(0)
            if NEW_LINK_MARKER in urllib.parse.urlparse(link).query:
                line_number = 1 + content[: m.start()].count("\n")
                links.append(f"{link} ({file_path}:{line_number})")

    if len(links) > 0:
        return links
    else:
        return None


def main() -> None:
    with ThreadPoolExecutor() as e:
        bad_files = [v for v in e.map(check_file, glob("**/*.md", recursive=True)) if v is not None]
        if len(bad_files) > 0:
            print(f"The following `?{NEW_LINK_MARKER}` URLs were found:")
            for file in bad_files:
                for link in file:
                    print(link)
            sys.exit(1)


if __name__ == "__main__":
    main()
