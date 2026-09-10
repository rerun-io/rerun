---
title: "`--asset` opens an asset together with a recording"
hidden: true
type: feature
---

### `rerun --asset`

`rerun rec.rrd --asset asset.rrd` registers the asset with the recording's dataset in the Viewer catalog, so the asset shows up in the recording.
Before this, an asset file written under its own recording id opened as a separate recording.

Every `--asset` is registered with all datasets that the specified local `.rrd` recordings end up in, regardless of argument order.
For example, `rerun --asset mesh.rrd rec0.rrd rec1.rrd --asset robot.rrd` registers both assets with each recording's dataset.
URLs, blueprints, and other non-recording arguments do not receive assets.
Passing `--asset` without any local `.rrd` recording produces an error.

`--asset` can be passed several times and needs **Load files via Viewer catalog** under **Settings** → **Viewer catalog** to be enabled, so it automatically enables the setting if it is off.

See [assets](../concepts/query-and-transform/catalog-object-model.md#assets) for what an asset is, and the [CLI manual](../reference/cli.md) for the full list of arguments.
