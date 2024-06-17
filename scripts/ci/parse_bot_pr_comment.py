#!/usr/bin/env python3
from __future__ import annotations

import os
import re
import sys

"""
Script for parsing @rerun-bot PR comments.

In order to avoid executing code that an attacker might embed in a comment,
the comment body is not passed as an argument, but instead through the `GITHUB_COMMENT_BODY` env
variable which is set by the GitHub Actions workflow.

Writes the desired content of `GITHUB_OUTPUT` to stdout.
"""

# List of valid inputs for the bot
valid_inputs = ["full-check"]


comment_body = os.environ.get("GITHUB_COMMENT_BODY", "")
bot_invocation = re.search(r"@rerun-bot\s+([a-zA-Z\-]*)", comment_body)
if bot_invocation:
    command = bot_invocation.group(1)
    if str(command) in valid_inputs:
        print(f"command={command}")
    else:
        print(f"Invalid command: {command}", file=sys.stderr)
