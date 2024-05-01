#!/usr/bin/env python3

"""Checks that no `.md` files contain invalid syntax."""

from __future__ import annotations

import re
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from glob import glob
from pathlib import Path


@dataclass
class Span:
    start: int
    end: int

    def offset(self, by: int):
        self.start += by
        self.end += by
        return self

    def slice(self, content: str) -> str:
        return content[self.start : self.end]


def extend_span(content: str, span: Span) -> Span:
    """
    Extends the span (start, end) to contain all lines it inhabits.

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

    if content[span.start] == "\n":
        actual_start = span.start + 1
    else:
        actual_start = content.rfind("\n", 0, span.start)
        if actual_start == -1:
            actual_start = 0

    if content[span.end] == "\n":
        actual_end = span.end
    else:
        actual_end = content.find("\n", span.end)
        if actual_end == -1:
            actual_end = len(content)

    return Span(start=actual_start, end=actual_end)


def get_line_num(content: str, start: int) -> int:
    return 1 + content[:start].count("\n")


@dataclass
class Error:
    message: str
    span: Span

    def render(self, filepath: str, content: str) -> str:
        span = extend_span(content, self.span)

        line_num = get_line_num(content, span.end)
        lines = span.slice(content).splitlines()
        line_num_align = len(f"{line_num + len(lines)}")

        out: list[str] = [f"{filepath}:{line_num}: {self.message}"]
        for line in lines:
            num = f"{line_num}".ljust(line_num_align)
            out.append(f"{num} | {line}")
            line_num += 1

        return "\n".join(out) + "\n"


@dataclass
class ElementSpans:
    element: Span
    """Span from the opening tag to the closing tag"""

    line: Span
    """Line span for the opening tag"""


def get_non_void_element_spans(content: str, search_start: int, tagname: str) -> ElementSpans | Error | None:
    element_start = content.find(f"<{tagname}", search_start)
    if element_start == -1:
        return None

    line_start = content.rfind("\n", 0, element_start)
    if line_start == -1:
        line_start = 0
    line_start += 1  # dont include the `\n` character

    line_end = content.find("\n", element_start)
    if line_end == -1:
        line_end = len(content)

    element_end = content.find(f"</{tagname}", element_start)
    if element_end == -1:
        return Error(message=f"<{tagname}> must have a closing tag", span=Span(line_start, line_end))

    return ElementSpans(
        element=Span(element_start, element_end),
        line=Span(line_start, line_end),
    )


def check_preceding_newline(tagname: str, content: str, spans: ElementSpans, errors: list[Error]):
    before_open_tag = Span(spans.line.start, spans.element.start).slice(content)
    if len(before_open_tag) == 0:
        # `<picture>` is not indented
        # walk to previous newline, then test if there is any non-whitespace inbetween
        ws_start = content.rfind("\n", 0, spans.line.start - 1)
        if ws_start == -1:
            ws_start = 0
        else:
            ws_start += 1
        ws_span = Span(ws_start, spans.element.start)

        if len(ws_span.slice(content).strip()) != 0:
            errors.append(
                Error(
                    f"<{tagname}> must be preceded by a blank line",
                    Span(ws_start, spans.line.end),
                )
            )


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


def check_picture_elements(content: str, errors: list[Error]) -> None:
    search_start = 0
    while True:
        spans = get_non_void_element_spans(content, search_start, "picture")
        if isinstance(spans, Error):
            errors.append(spans)
            return
        elif spans is None:
            return

        inner = spans.element.slice(content)
        for match in PICTURE_BAD_SOURCE_INDENT.finditer(inner):
            errors.append(
                Error(
                    "<picture> elements must not contain any blank lines",
                    Span(match.start(), match.end()).offset(spans.element.start),
                )
            )

        check_preceding_newline("picture", content, spans, errors)

        search_start = spans.element.end + 1


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
def check_video_elements(content: str, errors: list[Error]) -> None:
    search_start = 0
    while True:
        spans = get_non_void_element_spans(content, search_start, "video")
        if isinstance(spans, Error):
            errors.append(spans)
            return
        elif spans is None:
            return

        check_preceding_newline("video", content, spans, errors)

        search_start = spans.element.end + 1


def check_file(path: str) -> str | None:
    errors: list[Error] = []
    content = Path(path).read_text()

    check_picture_elements(content, errors)
    check_video_elements(content, errors)

    if len(errors) != 0:
        return "\n".join([error.render(path, content) for error in errors])


def main() -> None:
    with ThreadPoolExecutor() as e:
        errors = [v for v in e.map(check_file, glob("**/*.md", recursive=True)) if v is not None]
        if len(errors) > 0:
            print("The following invalid markdown files were found:\n")
            for error in errors:
                print(error)
            exit(1)
        print("No problems found")


if __name__ == "__main__":
    main()
