---
title: Opening files
order: 50
description: File types the Viewer and SDK open out of the box, and how to support more
---

The Rerun Viewer and SDK have built-in support for opening many kinds of files, and can be [extended](importers/overview.md) to support any other file type without needing to modify the Rerun codebase itself.

The Viewer can load files in 3 different ways:

-   via CLI arguments (e.g. `rerun myfile.jpeg`),
-   using drag-and-drop,
-   using the open dialog in the Rerun Viewer.

All these file loading methods support loading a single file, many files at once (e.g. `rerun myfiles/*`), or even folders.

> [!WARNING]
> Drag-and-drop of folders does [not yet work](https://github.com/rerun-io/rerun/issues/4528) on the web version of the Rerun Viewer.

The following data types have built-in support in the Rerun Viewer and SDK:

-   Native Rerun files: `rrd`, `rbl`
-   MCAP files: `mcap`
-   Robot descriptions: `urdf`
-   3D models: `dae`, `gltf`, `glb`, `obj`, `ply`, `stl`
-   Images: `avif`, `bmp`, `dds`, `exr`, `farbfeld`, `ff`, `gif`, `hdr`, `ico`, `jpeg`, `jpg`, `pam`, `pbm`, `pgm`, `png`, `ppm`, `tga`, `tif`, `tiff`, `webp`
-   Depth images: `rvl`
-   Video: `mp4`
-   Audio: `aac`, `flac`, `m4a`, `mp3`, `oga`, `ogg`, `wav`
-   Point clouds: `ply`
-   Parquet files: `parquet`
-   Text files: `md`, `txt`
-   [LeRobot](https://huggingface.co/docs/lerobot/index) datasets: `directory`

A `.ply` file can hold either a mesh or a point cloud, so it is listed twice: its header decides which one you get.
Point clouds may leave out `z`, in which case they load as 2D.

Files can also be opened from an HTTP URL (e.g. `rerun https://demo.rerun.io/version/latest/examples/dna/data.rrd`).
`rrd` and `rbl` files are streamed, while other supported formats are downloaded first and then loaded.

## Logging file contents from the SDK

To log the contents of a file from the SDK you can use the `log_file_from_path` and `log_file_from_contents` methods ([C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a8f253422a7adc2a19b89d1538c05bcac), [Python](https://ref.rerun.io/docs/python/stable/common/other_classes_and_functions/#rerun.log_file_from_path), [Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)) and the associated examples ([C++](https://github.com/rerun-io/rerun/blob/main/examples/cpp/log_file/main.cpp), [Python](https://github.com/rerun-io/rerun/blob/main/examples/python/log_file/log_file.py), [Rust](https://github.com/rerun-io/rerun/blob/main/examples/rust/log_file/src/main.rs)).

> [!NOTE]
> When calling these APIs from the SDK, the data will be loaded by the process running the SDK, not the Viewer!

snippet: tutorials/log-file

## Formats without a built-in importer

For other formats, read the file with your own code and log the data, or write an [importer](importers/overview.md) so the Viewer can open the file directly.

### CSV

There is no CSV importer.
Load the file with pandas (or polars), and log the table with [`rr.send_dataframe`](../send-data/send-dataframe.md):

```python
import pandas as pd
import pyarrow as pa
import rerun as rr

rr.init("rerun_example_csv", spawn=True)

df = pd.read_csv("data.csv")  # e.g. columns: frame, x, y
rr.send_dataframe(pa.Table.from_pandas(df), index="frame", entity_path="/data")
```

To show the values as points, boxes, or other archetypes, log them with [`send_columns`](../send-data/send-columns.md) instead.

### HDF5

There is no built-in HDF5 importer.
In Python, the experimental `rerun.experimental.Hdf5Reader` maps each HDF5 group to an entity and its datasets to components:

```python
import rerun as rr
from rerun.experimental import Hdf5Reader

rr.init("rerun_example_hdf5", spawn=True)

reader = Hdf5Reader("episode.hdf5")
rr.send_chunks(reader.stream())  # Rows are indexed on a generated `row_index` timeline.
```

All non-scalar datasets must have the same number of rows.
Pass `index_column=` to `stream()` to use one of the datasets as the timeline (e.g. `IndexColumn.sequence("/frame_id")` from `rerun.chunk`), and `ignore_datasets=` to skip datasets that don't fit.

Use `reader.groups()`, `reader.datasets()`, and `reader.attributes()` to inspect the file first.
You can also read the file with `h5py` and log the arrays yourself.

### Parquet

`.parquet` files have a built-in importer (see the list above).
For more control in Python, the experimental `rerun.experimental.ParquetReader` reads a file into a `LazyChunkStream` that you can reshape with [lenses](../../query-and-transform/lenses.md) before sending it with `rr.send_chunks`.
