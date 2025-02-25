#!/usr/bin/env bash
# Run stuff that often fails on CI

set -eux

echo "Removing old rrds…"
rm -f tests/assets/rrd/*.rrd

echo "Removing old output…"
rm -f docs/snippets/all/archetypes/*.rrd

echo "Generating .rrd files…"
pixi run -e py docs/snippets/compare_snippet_output.py --no-py --no-cpp

mv docs/snippets/all/archetypes/*_rust.rrd tests/assets/rrd

echo "Tracking .rrd files with Git LFS…"
git lfs track "*.rrd"

echo "Adding new .rrd files to Git…"
git add -f tests/assets/rrd/*.rrd
