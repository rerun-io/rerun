---
title: Migrating from 0.25 to 0.26
order: 984
---
<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## `cargo install rerun-cli` requires `protoc` (0.26.0 only)

**NOTE**: this has been fixed in 0.26.1 and no longer applies.

In order to install the Rerun CLI via cargo, you have to have a `protoc` installation on the `PATH` or `PROTOC` environment variable.

To install it on macOS, run `brew install protobuf`. It is also available at https://github.com/protocolbuffers/protobuf/releases. For more information see https://docs.rs/prost-build/#sourcing-protoc.

## Python SDK: removed `blocking` argument for `flush`
Use the new `timeout_sec` argument instead.
For non-blocking, use `timeout_sec=0`.
Mostly you can just call `.flush()` with no arguments.
That will block until all writes either finishes or an error occurs (e.g. the gRPC connection is severed).

## Python SDK: more use of kw-args
We have started using named arguments (kw-args) for more of our functions.
This will make it easier for us to evolve our APIs in the future, when adding new arguments, or renaming old ones.

Before:
```py
rr.ImageFormat(width, height, "YUV420")

blueprint.spawn("my_app", 1234)
```

After:
```py
rr.ImageFormat(width, height, pixel_format="YUV420")

blueprint.spawn("my_app", port=1234)
```

[ruff](https://github.com/astral-sh/ruff) (or your preferred Python linter) will find all the places in your code that need to be updated!

## Python DataFusion interface: update to 49.0.0
The DataFusion FFI that we rely on for user defined functions and
table providers requires users to upgrade their `datafusion-python`
version to 49.0.0. This only impacts customers who use the
DataFusion tables provided through the `CatalogClient`.


## Partition table changes and new dataset manifest

The partition table used to contain a lot of information about the underlying layers (of which there may be several per partition).
This caused unnecessary noise and some tooling problems due to the complex Arrow schema.

To address that, the partition table has been simplified with multiple columns removed and a few others renamed.
In parallel, a new dataset manifest table is now available (`dataset_entry.manifest()`).
This table contains one row per layer (i.e. possibly multiple rows per partition) and provide a rich low-level view on the contents of a dataset.

#### Partition table

- `rerun_partition_id`: partition id (string)
- `rerun_layer_names`: layer names (list of strings, one value per layer)
- `rerun_storage_urls`: layer storage urls (list of strings, one value per layer)
- `rerun_last_updated_at`: last update of the partition (nanoseconds timestamp)
- `rerun_num_chunks`: total number of chunks in the partition (uint64)
- `rerun_size_bytes`: total number of bytes for the partition (uint64)
- `property:*`: properties columns derived from the partition data


#### Dataset manifest columns

- `rerun_layer_name`: layer name (string)
- `rerun_partition_id`: partition id (string)
- `rerun_storage_url`: layer storage url (string)
- `rerun_layer_type`: layer type (string)
- `rerun_registration_time`: registration time (nanoseconds timestamp)
- `rerun_last_updated_at`: last update of the layer (nanoseconds timestamp)
- `rerun_num_chunks`: number of chunks in the layer (uint64)
- `rerun_size_bytes`: number of bytes for the layer (uint64)
- `rerun_schema_sha256`: sha256 of the layer schema (fixed width binary, size = 32 bytes)
- `property:*`: properties columns derived from the layer data
