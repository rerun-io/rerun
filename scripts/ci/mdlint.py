#!/usr/bin/env python3

"""
Checks that no `.md` files contain invalid syntax.

Usage:
  python mdlint.py
  python mdlint.py explain e001
  python mdlint.py lint docs/content/**/*.md
"""

from __future__ import annotations

import argparse
import re
import sys
import textwrap
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from glob import glob
from pathlib import Path
from typing import Self


@dataclass
class Span:
    start: int
    end: int

    def offset(self, by: int) -> Self:
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
    code: str
    message: str
    span: Span

    def render(self, filepath: str, content: str) -> str:
        span = extend_span(content, self.span)

        line_num = get_line_num(content, span.end)
        lines = span.slice(content).splitlines()
        line_num_align = len(f"{line_num + len(lines)}")

        out: list[str] = [
            f"error[{self.code}]: {self.message}",
            f"{' '.rjust(line_num_align)}--> {filepath}:{line_num}",
        ]
        for line in lines:
            num = f"{line_num}".ljust(line_num_align)
            out.append(f"{num} | {line}")
            line_num += 1

        return "\n".join(out) + "\n"


class NoClosingTagError(Error):
    CODE = "E001"

    def __init__(self, tagname: str, span: Span) -> None:
        super().__init__(type(self).CODE, f"<{tagname}> has no closing tag", span)

    @staticmethod
    def explain() -> str:
        return textwrap.dedent(
            """
            If a block element such as `<video>` has no closing tag,
            then it may not render correctly.

            Example:
            ```
            <video>
              <source>
                        ## no closing tag
            ```

            Solution: Add a closing tag.
            ```
            <video>
              <source>
            </video>
            ```
            """,
        )


class NoPrecedingBlankLineError(Error):
    CODE = "E002"

    def __init__(self, tagname: str, ws_start: int, line_end: int) -> None:
        super().__init__(type(self).CODE, f"<{tagname}> is not preceded by a blank line", Span(ws_start, line_end))

    @staticmethod
    def explain() -> str:
        return textwrap.dedent(
            """
            If a block element such as `<video>` is preceded by a paragraph
            without a blank line between the text and the tag, then the
            element is considered part of the text.

            Example:
            ```
            Some text:
            <video>      ## no blank line
              <source>
            </video>
            ```

            The `source` and closing `video` tag in the above snippet
            are ignored by the markdown renderer, resulting in a broken video.

            Solution: Add a blank line after the text.
            ```
            Some text:
                         ## blank line added here
            <video>
              <source>
            </video>
            ```
            """,
        )


class BlankLinesError(Error):
    CODE = "E003"

    def __init__(self, tagname: str, span: Span) -> None:
        super().__init__(type(self).CODE, f"<{tagname}> element contains a blank line", span)

    @staticmethod
    def explain() -> str:
        return textwrap.dedent(
            """
            If a block element such as `<picture>` contains a blank line,
            then everything after the blank line is treated as part of a
            separate element.

            Example:
            ```
            <picture>
                            ## blank line
              <source>
            </picture>
            ```

            The `source` and closing `picture` tag in the above snippet
            are ignored by the markdown renderer, resulting in a broken image.

            Solution: Remove the blank line, and ensure all content has proper indentation.
            ```
            <picture>
              <source>      ## blank line is gone
            </picture>
            ```
            """,
        )


class BacktickLinkError(Error):
    CODE = "E004"

    def __init__(self, span: Span) -> None:
        super().__init__(type(self).CODE, "link contains backtick", span)

    @staticmethod
    def explain() -> str:
        return textwrap.dedent(
            """
            URLs in links wrapping text should not contain backticks (`).

            Example:
            ```
            [Some link](`https://github.com/rerun-io/rerun`)
            ```

            Our markdown renderer will treat the above link as a _relative path_
            instead of a URL. If the above markdown is in `examples/robotics/README.md`,
            it will link to \"https://rerun.io/examples/robotics/`https://github.com/rerun-io/rerun`\".

            Solution: Remove the backticks.
            ```
            [Some link](https://github.com/rerun-io/rerun)
            ```
            """,
        )


class BadDataReferenceError(Error):
    CODE = "E005"

    def __init__(self, data_reference_name: str, span: Span) -> None:
        super().__init__(type(self).CODE, f"`{data_reference_name}` is not a valid data reference", span)

    @staticmethod
    def explain() -> str:
        return textwrap.dedent(
            """
            A `data_inline_viewer` should be a valid reference.
            """,
        )


EXPLAIN = {
    NoClosingTagError.CODE: NoClosingTagError.explain,
    NoPrecedingBlankLineError.CODE: NoPrecedingBlankLineError.explain,
    BlankLinesError.CODE: BlankLinesError.explain,
    BacktickLinkError.CODE: BacktickLinkError.explain,
    BadDataReferenceError.CODE: BadDataReferenceError.explain,
}


@dataclass
class ElementSpans:
    opening_tag_content: Span
    """Span for the opening tag content"""

    element: Span
    """Span from the opening tag to the closing tag"""

    line: Span
    """Line span for the opening tag"""


def get_non_void_element_spans(content: str, search_start: int, tagname: str) -> ElementSpans | Error | None:
    element_start = content.find(f"<{tagname}", search_start)
    if element_start == -1:
        return None

    opening_tag_content_start = element_start + len(tagname) + 1
    opening_tag_content_end = content.find(">", opening_tag_content_start)

    line_start = content.rfind("\n", 0, element_start)
    if line_start == -1:
        line_start = 0
    line_start += 1  # dont include the `\n` character

    line_end = content.find("\n", element_start)
    if line_end == -1:
        line_end = len(content)

    element_end = content.find(f"</{tagname}", element_start)
    if element_end == -1:
        return NoClosingTagError(tagname, Span(line_start, line_end))

    return ElementSpans(
        opening_tag_content=Span(opening_tag_content_start, opening_tag_content_end),
        element=Span(element_start, element_end),
        line=Span(line_start, line_end),
    )


def check_preceding_newline(tagname: str, content: str, spans: ElementSpans, errors: list[Error]) -> None:
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
            errors.append(NoPrecedingBlankLineError(tagname, ws_start, spans.line.end))


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

DATA_INLINE_VIEWER = "data-inline-viewer"


def check_picture_elements(content: str, errors: list[Error]) -> None:
    search_start = 0
    while True:
        spans = get_non_void_element_spans(content, search_start, "picture")
        if spans is None:
            return
        elif isinstance(spans, Error):
            errors.append(spans)
            return

        inner = spans.element.slice(content)
        for match in PICTURE_BAD_SOURCE_INDENT.finditer(inner):
            errors.append(BlankLinesError("picture", Span(match.start(), match.end()).offset(spans.element.start)))

        check_preceding_newline("picture", content, spans, errors)

        tag_content = spans.opening_tag_content.slice(content)
        data_inline_viewer = tag_content.find(DATA_INLINE_VIEWER)

        if data_inline_viewer >= 0:
            data_reference_start = tag_content.find('"', data_inline_viewer + len(DATA_INLINE_VIEWER)) + 1
            data_reference_end = tag_content.find('"', data_reference_start)

            data_reference_span = Span(data_reference_start, data_reference_end)
            data_reference_name = data_reference_span.slice(tag_content)
            (kind, name) = data_reference_name.split("/", 1)

            valid_reference = (kind == "snippets" and glob(f"docs/snippets/all/{name}.py")) or (
                kind == "examples" and glob(f"examples/python/{name}")
            )

            if not valid_reference:
                errors.append(BadDataReferenceError(data_reference_name, data_reference_span))

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
        if spans is None:
            return
        elif isinstance(spans, Error):
            errors.append(spans)
            return

        check_preceding_newline("video", content, spans, errors)

        search_start = spans.element.end + 1


def check_invalid_links(content: str, errors: list[Error]) -> None:
    search_start = 0
    while True:
        mid_point = content.find("](`", search_start)
        if mid_point == -1:
            return

        link_start = content.rfind("[", 0, mid_point)
        if link_start == -1:
            # TODO(jprochazk): invalid link
            search_start = mid_point
            continue

        link_end = content.find(")", mid_point)
        if link_end == -1:
            # TODO(jprochazk): invalid link
            search_start = mid_point
            continue

        search_start = link_end + 1

        errors.append(BacktickLinkError(span=Span(link_start, link_end)))


def check_file(path: str) -> str | None:
    errors: list[Error] = []
    content = Path(path).read_text(encoding="utf-8")

    check_picture_elements(content, errors)
    check_video_elements(content, errors)
    check_invalid_links(content, errors)

    if len(errors) != 0:
        return "\n".join([error.render(path, content) for error in errors])
    return None


def lint(glob_pattern: str) -> None:
    with ThreadPoolExecutor() as e:
        errors = [v for v in e.map(check_file, glob(glob_pattern, recursive=True)) if v is not None]
        if len(errors) > 0:
            print("The following invalid markdown files were found:\n")
            for error in errors:
                print(error)
            sys.exit(1)
        print("No problems found")


def explain(error_code: str) -> None:
    if error_code not in EXPLAIN:
        print(f'Unknown error code "{error_code}"')
        print(f"Available error codes: {', '.join(EXPLAIN.keys())}")
        return

    f = EXPLAIN[error_code]
    print(f())


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Markdown linter")
    parser.add_argument(
        "--glob",
        type=str,
        default="**/*.md",
        help="glob pattern of files to lint, e.g. '**/*.md'",
    )
    parser.add_argument(
        "--explain",
        type=str,
        help="explain an error code",
    )

    return parser.parse_args()


def main() -> None:
    args = parse_args()

    if args.explain is not None:
        explain(args.explain.upper())
    else:
        lint(args.glob)


if __name__ == "__main__":
    main()
