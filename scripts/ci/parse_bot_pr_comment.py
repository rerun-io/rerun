#!/usr/bin/env python3
from __future__ import annotations

import os
import re

"""
Script for parsing @rerun-bot PR comments.

In order to avoid executing code that an attacker might embed in a comment,
the comment body is not passed as an argument, but instead through the `GITHUB_COMMENT_BODY` env
variable which is set by the GitHub Actions workflow.
"""

# List of valid inputs for the bot
valid_inputs = ["full-check"]


def gh_set_output(key: str, text: str) -> None:
    line = f"{key}={text}"
    output_file = os.environ.get("GITHUB_OUTPUT")
    if output_file is not None:
        # append to output_file
        with open(output_file, "a", encoding="utf8") as f:
            f.write(f"{line}{os.linesep}")
    else:
        # print when not running on CI
        print(line)


def main() -> None:
    comment_body = os.environ.get("GITHUB_COMMENT_BODY", "")
    bot_invocation = re.search(r"@rerun-bot\s+([a-zA-Z\-]*)", comment_body)

    if bot_invocation:
        command = bot_invocation.group(1)
        if str(command) in valid_inputs:
            print(f"Valid command found: {command}")
            gh_set_output("command", command)
        else:
            print(f"Invalid command: {command}")
    else:
        print("No rerun-bot invocation found")


if __name__ == "__main__":
    main()
