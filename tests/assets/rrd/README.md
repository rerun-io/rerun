`.rrd` files that are checked in to `git lfs`.
We use this to ensure we can still load old `.rrd` files.

To update the contents of this folder, run:

> tests/assets/rrd/generate-compatibility-rrds.sh

To verify that they all still load, run:

> pixi run rerun rrd verify tests/assets/rrd/*.rrd
