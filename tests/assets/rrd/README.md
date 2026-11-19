`.rrd` files that are checked in to `git lfs`. We use this to ensure we can still load old `.rrd` files.

We guarantee that `.rrd` files from Rerun 0.23 and later load in any newer version, and these files check that we keep that promise.
See [RRD file backwards compatibility](../../../docs/content/logging-and-ingestion/tutorial.md#rrd-file-backwards-compatibility).

### Verifying
To verify that they all still load, run:

> pixi run check-backwards-compatibility


### Updating
To update the contents of this folder, run:

> tests/assets/rrd/generate-compatibility-rrds.sh
