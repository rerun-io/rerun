# Registering: view, register, query

Data files are there to be used: view, register, and query.
Provide the commands and scripts that do all these. Write them in the dataset's README as needed.

## View

The viewer takes the layers as separate arguments, and the blueprint alongside them:

```bash
rerun <filename>.rrd                               # a single .rrd
rerun <base>.rrd <layer1>.rrd … <blueprint>.rbl    # base plus extra layers and the blueprint
```

## Register

Registration puts the recordings in a catalog so they can be queried as a dataset.
Start the server first:

```bash
rerun serve # let it running
```

Then register each layer.
Registration has no CLI: it is a Python API, so it lives in a script you write.
Where that script goes is a project decision. Propose the shape that fits the project's layout and let the user confirm it.

```bash
# Example shape only — the project's own script, run after the .rrd files are written.
python scripts/register.py --rrd-dir out/ --dataset my_dataset
```

Pick the registration method by where the files are: `register` for an explicit list of URIs, `register_prefix` for files already under one prefix in object storage.

### Registering a list of files

The script itself makes these calls:

```python
from rerun.catalog import CatalogClient, OnDuplicateSegmentLayer

# A Rerun server must be running at this URL; 51234 is the default port.
client = CatalogClient("rerun+http://127.0.0.1:51234")
dataset = client.create_dataset("my_dataset", exist_ok=True)

# Each .rrd registers as one layer of a segment; the segment is keyed by the
# recording_id stored inside the .rrd, so files sharing an id stack as layers.
dataset.register(
    ["file:///path/to/file.rrd"],
    layer_name="base",
    on_duplicate=OnDuplicateSegmentLayer.REPLACE,
).wait()

# Optional: install a default blueprint for the dataset.
dataset.register_blueprint("file:///path/to/file.rbl", set_default=True)
```

Three details:

- **Paths are absolute `file://` URIs.** A relative path fails, so build the URI from a resolved path rather than pasting one by hand.
- **`REPLACE` makes re-registration idempotent.** Without it a second run of the same conversion errors on the layers already there.
- **The server URL can be remote.** `rerun+http://127.0.0.1:51234` is the local default; a hosted catalog takes its own `rerun+https://…` URL.
  What you register must be readable by that server, so a remote catalog needs the files in object storage (`s3://…`) rather than on your disk.
  See [Staging files](#staging-files).

Call `register` once per layer, with the layer name each set of files was built as.
Registering the URDF layer under `layer_name="base"` silently shadows the base layer.

### Staging files

Staging puts a local file in the catalog's storage so the server can register it.

- rerun-sdk 0.39 or later: `client.stage(path, key=…)` uploads the file and returns the URI to register.
  The server must also support staging; otherwise the call raises `RuntimeError`.
- Earlier versions: copy the file to a bucket the server can read with the storage provider's tools (e.g. `aws s3 cp`) and register its `s3://…` URI.

Use a key that is unique to the segment and layer, since Rerun Hub may overwrite an object that has the same key.

### Registering a prefix

For up to about 50,000 RRDs already in object storage, `dataset.register_prefix(prefix, layer_name, on_duplicate=…)` sends the whole prefix to the server in one request.
This only supports one layer name per call, so prefer to organize each layer under its own prefix.

A `register_prefix` call over too many files fails with `encoded message length too large`.
Past about 50,000 files, list the URIs yourself and call `register` in batches of fewer than 50,000 URIs instead:

- Run a few batches at the same time, and read each batch's results with `handle.iter_results(timeout_secs=…)`, so a failure is reported against its own batch.
- The server rejects a whole request when one RRD in it is unreadable.
  Retry a rejected batch in halves to isolate the bad file and let the rest through.
- Make the run resumable: skip the URIs already listed in the `rerun_storage_urls` column of `dataset.segment_table()`.

### Registering from distributed workers

When distributed workers each write one segment's layer (see `post-processing.md`), each worker can register its own file right after staging it with a single-URI `register` call and `REPLACE`, so a retried worker stays idempotent.

> **Caution:** Concurrent registrations are still improving; too many at once may slow down Rerun Hub.
> For a large-scale run, let the workers only write and stage.
> Once all workers are done, aggregate the staged URIs and register them at once as described in [Registering a prefix](#registering-a-prefix).

## View catalog

Once a server is running and a dataset is registered, point the Viewer at your server to browse every recording in the catalog.

```bash
rerun rerun+http://127.0.0.1:51234  # This is the default server URL
```

Here too the URL is only the local default; point it at a remote catalog to browse that one instead.

## Query

Once a server is running and a dataset is registered, the dataset can be queried.
There is no general query that must be done. Depending on user's interest,
use `rerun-catalog-queries` skill (`CatalogClient` → `dataset.reader(…)` → a DataFusion `DataFrame`) to find interesting results.
Read it before shaping a per-episode pipeline, and before assuming a slow query is the catalog's fault.

## Further notes

The project's README and register script should carry real names and paths.
It should include the view command with its actual layer files, the registration script or the command that runs it, and working query examples.
