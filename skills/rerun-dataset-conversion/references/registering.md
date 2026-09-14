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

Call `register` once per layer, with the layer name each set of files was built as.
Registering the URDF layer under `layer_name="base"` silently shadows the base layer.

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

The project's README and register script should carry real names and paths. The view command with its actual layer files, the registration script or the command that runs it, and working query example(s).
