#!/usr/bin/env python3

"""Demonstrates the most barebone usage of the Rerun SDK."""

import rerun as rr

rr.spawn()

rr.log_point("my_point", [1, 1, 1])
