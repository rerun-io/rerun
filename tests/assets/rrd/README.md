`.rrd` files that are checked in to `git lfs`. We use this to ensure we can still load old `.rrd` files.

We don't yet guarantee backwards compatibility, but we at least check it so that we _know_ if/when we break it.

### Verifying
To verify that they all still load, run:

> pixi run rerun rrd verify tests/assets/rrd/*.rrd


### Updating
To update the contents of this folder, run:

> tests/assets/rrd/generate-compatibility-rrds.sh
