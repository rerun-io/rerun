#!/usr/bin/env bash
# Generate and check in .rrd files used for backwards compatibility tests.

set -eux


SOURCE_DIR="docs/snippets/all"
DEST_DIR="tests/assets/rrd"


echo "Removing old rrds…"
rm -f "${DEST_DIR}/*.rrd"


echo "Generating example .rrd files…"
pixi run -e examples build-examples rrd --channel main ${DEST_DIR}


echo "Removing old snippet output…"
find "${SOURCE_DIR}" -type f -name "*.rrd" -exec rm -f {} +

echo "Generating snippet .rrd files…"
pixi run -e py docs/snippets/compare_snippet_output.py --no-py --no-cpp

echo "Copying .rrd files to ${DEST_DIR}…"
find "$SOURCE_DIR" -type f -name "*.rrd" -exec cp {} "${DEST_DIR}" \;


echo "Tracking .rrd files with git LFS…"
git lfs track "*.rrd"


echo "Adding new .rrd files to git…"
git add -f ${DEST_DIR}/*.rrd
