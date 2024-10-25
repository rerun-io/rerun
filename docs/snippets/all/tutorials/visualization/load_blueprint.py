"""Demonstrates how to programmatically re-use a blueprint stored in a file."""

import sys

import rerun as rr

path_to_rbl = sys.argv[1]

rr.init("rerun_example_reuse_blueprint_file", spawn=True)
rr.log_file_from_path(path_to_rbl)

# … log some data as usual …
