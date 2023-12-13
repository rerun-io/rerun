#!/usr/bin/env python3
"""
Demonstrates how to log data to standard output with the Rerun SDK, and then visualize it
from standard input with the Rerun Viewer.
"""
from __future__ import annotations

import sys

import rerun as rr  # pip install rerun-sdk

# sanity-check since all other example scripts take arguments:
assert len(sys.argv) == 1, f"{sys.argv[0]} does not take any arguments"

rr.init("rerun_example_stdio")
rr.stdout()

input = sys.stdin.buffer.read()

rr.log("stdin", rr.TextDocument(input.decode('utf-8')))
