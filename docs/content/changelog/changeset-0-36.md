---
title: "0.36"
order: 974
---

## Highlights

Rerun 0.36 brings 3D gaussian splats to the Viewer: log them from the SDK or open a PLY file directly, complete with anisotropic scale, opacity, and view-dependent color.
The experimental Viewer catalog now streams `.rrd` files lazily in the browser, so the web Viewer can open recordings larger than your RAM.
3D views got configurable axes, the timeline got a new playhead navigation menu, and `GrpcServerSink` can finally be combined with other sinks, so you can serve live data and write a file at the same time.

### Experimental 3D gaussian splat support

Rerun has now an experimental [`GaussianSplats3D`](../reference/types/archetypes/gaussian_splats3d.md) archetype and supports visualizing them in the Viewer!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/2ef9455946158cc698f679b72782ddc9a8c8ff44_splat-training.mp4" type="video/mp4" />
</video>

You can load any gaussian splat PLY file directly with the viewer or log the archetype directly from the logging SDK.
The archetype supports anisotropic scale and rotation, opacity, and view-dependent color using spherical harmonics up to degree 3.

This support is experimental as the archetype and our renderer (which is kept very simple for now and is compatible with all browsers) may still evolve significantly.

Let us know which formats and splatting variants you would like us to support next!

### Experimental Viewer catalog: larger-than-RAM files on the web

The [experimental Viewer catalog](changeset-0-35.md#experimental-viewer-catalog) can now load `.rrd` files lazily in the web Viewer.
Files opened through the file dialog or drag-and-drop are streamed into the browser's Origin Private File System without passing through Wasm linear memory, and recording chunks are then read on demand.
This lets the web Viewer open recordings larger than available RAM and even larger than Wasm's 4 GiB address space.
Reopening the same file reuses its content-addressed browser-storage copy, when persistence is enabled.

Embedded default blueprints are now preserved when an `.rrd` is registered, so recordings open with their intended layout.
The Viewer catalog also stays hidden in the recording panel until it contains data.

To try it, open **Settings**, then enable **Load files via Viewer catalog** under **Experimental** before opening the `.rrd` file.
For large files on the web, also select **Request persistence** under **Origin private filesystem**.
This asks the browser to protect Viewer catalog files from automatic storage eviction and may increase the storage quota in some browsers; the browser can still deny the request.

### Configurable axes for 3D views

Previously, setting the up, left, and down directions of a 3D scene required logging [`ViewCoordinates`](../reference/types/archetypes/view_coordinates.md) at the scene root.
You can now configure the axes directly from the UI or through blueprint code by changing the `axes` property.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/9b58e17322e9aa061e728be80762d7474709c5bd_axes-controls.mov" type="video/quicktime" />
</video>

### Improved viewer timeline navigation

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/6a838cb2d11b2657d877ba357ac87e09c2efd9ab_timeline navigation.mp4" type="video/mp4" />
</video>

A new playhead navigation menu makes it easier to jump through a recording, and shows you what keyboard shortcuts are available.

### Multi-sink support for `GrpcServerSink`

`GrpcServerSink` can now be combined with other recording sinks in the Rust, Python, and C++ SDKs.
This makes it possible to stream data to connected Viewers while simultaneously writing the same recording elsewhere.

```python
# Stream data to several sinks at once:
rr.set_sinks(
    # Host a gRPC proxy server that web Viewers can connect to:
    rr.GrpcServerSink(),
    # Write data to a `data.rrd` file in the current directory:
    rr.FileSink("data.rrd"),
)
```

See [Multiple sinks](../concepts/logging-and-ingestion/sinks.md#multiple-sinks-tee-pattern) for more information.

### Better MCAP file introspection via CLI and Python

The `rerun mcap info` CLI command has been rewritten to output richer and more detailed file-level information instead of just diagnostic checks.
The diagnostic checks are now in a dedicated `rerun mcap check` subcommand instead.
For programmatic access, the same information can be now also accessed through `McapReader.info()` in Python:

```python
reader = McapReader("data.mcap")

print(f"{reader.info().chunks.count} chunks at max. {reader.info().chunks.max_compressed_size_bytes} B")

if reader.info().metadata_count > 0:
    print("MCAP has metadata records")

for channel in reader.info().channels:
    print(f"{channel.topic} [{channel.schema.name}] - {channel.message_count}")
```

The info is lazily built & cached once, so repeated calls to `info()` are cheap.

### Optimize a recording from Rust with `rrd::optimize`

Optimizing a recording is now available from Rust as well, behind the `rrd` feature (enabled by default).
Previously this was only possible through the CLI and Python.

`optimize` reads from any reader and writes to any writer, so the output can go straight to a memory buffer, a socket, or an object store upload:

```rust
let profile = rerun::rrd::OptimizationProfile::OBJECT_STORE;

let input = std::io::Cursor::new(std::fs::read("input.rrd")?);
let mut output: Vec<u8> = Vec::new();
let stats = rerun::rrd::optimize(input, &mut output, &profile)?;
println!("wrote {} chunks, {} bytes of messages", stats.num_chunks, stats.num_bytes);
```

`optimize_file` is the shorthand for the file-to-file case.
It reads the input in full before it truncates the output, so both may be the same file:

```rust
rerun::rrd::optimize_file("input.rrd", "output.rrd", &profile)?;
```

Optimize currently holds the whole recording in memory while it works.

This change also corrects the version stamped on the output of `rerun rrd optimize` and `rerun rrd merge` when the input holds more than one store.
They used to take the version of whichever store came first in a hash map, and fall back to the writing version when that store declared none.
They now take the newest version across all the input stores.

Available from 0.36.3.
See [Optimize chunk count](../howto/logging-and-ingestion/optimize-chunks.md) for what the optimization profiles do and when to reach for them.

## Breaking changes

### Entry-name restrictions now apply to application IDs

To unify application IDs and [catalog entry names](../concepts/query-and-transform/catalog-object-model.md#catalog), the `EntryName` restrictions now also apply to [`ApplicationId`](../concepts/logging-and-ingestion/recordings.md#application-ids).
Rerun tries to migrate existing application IDs by replacing unsupported characters and dots with hyphens and adding a short hash suffix.
Long application IDs are truncated and receive the same suffix.

Update application IDs to use at most 180 characters and only ASCII alphanumeric characters, underscores, hyphens, spaces, brackets, and colons.
For example, change `my/application` to `my-application`.

### `ParquetReader` loading options moved to `stream()`

The experimental `ParquetReader`'s constructor now takes only the file path.
All loading options (`entity_path_prefix`, `column_grouping`, `delimiter`, `prefixes`, `use_structs`, `static_columns`, `index_columns`) moved to `stream()`:

```python
# 0.35
ParquetReader(path, column_grouping="individual", index_columns=[IndexColumn.sequence("frame")]).stream()

# 0.36
ParquetReader(path).stream(column_grouping="individual", index_columns=[IndexColumn.sequence("frame")])
```

The reader is now a lightweight handle over the file, and each `stream()` call is independent — one reader can drive several differently-configured streams over the same file.

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
