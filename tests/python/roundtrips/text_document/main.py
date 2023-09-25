#!/usr/bin/env python3

"""Logs a `TextDocument` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import rerun as rr
import rerun.experimental as rr2


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_text_document")

    rr2.log("text_document", rr2.TextDocument("Hello, TextDocument!"))
    rr2.log(
        "markdown",
        rr2.TextDocument(
            body="# Hello\nMarkdown with `code`!\n\nA random image:\n\n![A random image](https://picsum.photos/640/480)",
            media_type="text/markdown",
        ),
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
