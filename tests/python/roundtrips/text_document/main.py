#!/usr/bin/env python3

"""Logs a `TextDocument` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_text_document")

    rr.log("text_document", rr.TextDocument("Hello, TextDocument!"))
    rr.log(
        "markdown",
        rr.TextDocument(
            body="# Hello\nMarkdown with `code`!\n\nA random image:\n\n![A random image](https://picsum.photos/640/480)",
            media_type=rr.MediaType.MARKDOWN,
        ),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
