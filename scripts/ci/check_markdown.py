#!/usr/bin/env python3

"""Checks that no `.md` files contain invalid syntax."""

from __future__ import annotations

import re
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from glob import glob
from pathlib import Path


@dataclass
class BadFile:
    path: str
    error: str


def extend_span(content: str, start: int, end: int) -> tuple[int, int]:
    """
    Extends the span (start, end) to contain the full content of all lines it inhabits.

    For example (with `|` denoting span bounds):
    ```
    <pictu|re>
      <so|urce>
    ```

    Turns into:
    ```
    |<picture>
      <source>|
    ```
    """

    actual_start = content.rfind("\n", 0, start)
    if actual_start == -1:
        actual_start = 0

    actual_end = content.find("\n", end)
    if actual_end == -1:
        actual_end = len(content)

    return actual_start, actual_end


def render_error(path: str, error: str, content: str, span: tuple[int, int]) -> str:
    start, end = span
    start, end = extend_span(content, start, end)
    line_num = 1 + content[:start].count("\n")
    lines = content[start:end].splitlines()
    line_num_align = len(f"{line_num + len(lines)}")

    out: list[str] = [f"{path}:{line_num}: {error}"]
    for line in lines:
        num = f"{line_num}".ljust(line_num_align)
        out.append(f"{num} | {line}")
        line_num += 1

    return "\n".join(out) + "\n"


def get_line_num(content: str, start: int) -> int:
    return 1 + content[:start].count("\n")


# example:
#   <picture>
#               <-- newline between tags
#     <source>
#   </picture>
#
# solution:
#   <picture>
#     <source>
#   </picture>
PICTURE_BAD_SOURCE_INDENT = re.compile("(\\n\\s*)(\\n\\s+)<source")


def check_picture_indentation(path: str, content: str, errors: list[str]) -> None:
    end = 0
    while True:
        start = content.find("<picture", end)
        if start == -1:
            return

        end = content.find("</picture", start)
        if end == -1:
            line_num = get_line_num(content, start)
            errors.append(f"{path}:{line_num}: <picture> without closing tag")
            return
        else:
            inner = content[start:end]
            for match in PICTURE_BAD_SOURCE_INDENT.finditer(inner):
                span_start, span_end = match.span()
                span_start += start
                span_end += start
                errors.append(render_error(path, "invalid <source> indentation", content, (span_start, span_end)))


# example:
#   some text:
#   <video>     <-- no newline before this html tag
#     <source>
#   </video>
#
# solution:
#   some text:
#
#   <video>
#     <source>
#   </video>


def check_video_element(path: str, content: str, errors: list[str]) -> None:
    end = 0
    while True:
        start = content.find("<video", end)
        if start == -1:
            return

        end = content.find("</video", start)


def check_file(path: str) -> str | None:
    errors: list[str] = []
    content = Path(path).read_text()

    check_picture_indentation(path, content, errors)

    if len(errors) != 0:
        return "\n".join(errors)


def main() -> None:
    with ThreadPoolExecutor() as e:
        errors = [v for v in e.map(check_file, glob("**/*.md", recursive=True)) if v is not None]
        if len(errors) > 0:
            print("The following invalid markdown files were found:")
            for error in errors:
                print(error)
            exit(1)


if __name__ == "__main__":
    main()
