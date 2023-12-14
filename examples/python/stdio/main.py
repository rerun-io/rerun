#!/usr/bin/env python3
"""
Demonstrates how to use standard input/output with the Rerun SDK/Viewer.

Usage: `echo 'hello from stdin!' | python main.py | rerun -`
"""
from __future__ import annotations

import sys

import rerun as rr  # pip install rerun-sdk

# sanity-check since all other example scripts take arguments:
assert len(sys.argv) == 1, f"{sys.argv[0]} does not take any arguments"

rr.init("rerun_example_stdio")
rr.stdout()

input = sys.stdin.buffer.read()

rr.log("stdin", rr.TextDocument(input.decode("utf-8")))
