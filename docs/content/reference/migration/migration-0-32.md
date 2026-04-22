---
title: Migrating from 0.31 to 0.32
order: 978
---

## "Data loaders" renamed to "importers"

The file import system previously called "data loaders" has been renamed to "importers" to avoid confusion with the widely-used ML/PyTorch "dataloader" concept and to better describe what the system does: importing external file formats into Rerun.

The old names are deprecated but most of them still work for this release.

### Rust API

| Before | After |
|--------|-------|
| `rerun::DataLoader` | `rerun::Importer` |
| `rerun::DataLoaderSettings` | `rerun::ImporterSettings` |
| `rerun::DataLoaderError` | `rerun::ImporterError` |
| `rerun::LoadedData` | `rerun::ImportedData` |
| `rerun::EXTERNAL_DATA_LOADER_PREFIX` | `rerun::EXTERNAL_IMPORTER_PREFIX` |
| `rerun::EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE` | `rerun::EXTERNAL_IMPORTER_INCOMPATIBLE_EXIT_CODE` |

All old names are available as deprecated type aliases and will be removed in a future release.

The Cargo feature flag `data_loaders` has been renamed to `importers`.

### C/C++ API

| Before | After |
|--------|-------|
| `rr_data_loader_settings` | `rr_importer_settings` |
| `EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE` | `EXTERNAL_IMPORTER_INCOMPATIBLE_EXIT_CODE` |

The old names are still available as deprecated aliases.

### Python API

| Before | After |
|--------|-------|
| `rr.EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE` | `rr.EXTERNAL_IMPORTER_INCOMPATIBLE_EXIT_CODE` |

The old name is still available but deprecated.

### External importers

Executables on `$PATH` with the `rerun-importer-` prefix are now the canonical way to register external importers. The old `rerun-loader-` prefix continues to work but will log a deprecation warning.

Before:

```
rerun-loader-my-format
```

After:

```
rerun-importer-my-format
```

## Lenses API (Rust)

### `output_columns_at` / `output_scatter_columns_at` replaced by `OutputBuilder::at_entity`

The `output_columns_at` and `output_scatter_columns_at` methods on `LensBuilder` have been removed.
Use `at_entity` on the `OutputBuilder` inside the closure instead:

```rust
// Before
Lens::for_input_column(filter, "component")
    .output_columns_at("target/entity", |out| {
        out.component(descriptor, selector)
    })?

// After
Lens::for_input_column("component")
    .output_columns(|out| {
        out.at_entity("target/entity")
            .component(descriptor, selector)
    })?
```

The same applies to scatter columns:

```rust
// Before
.output_scatter_columns_at("target/entity", |out| { … })?

// After
.output_scatter_columns(|out| {
    out.at_entity("target/entity") // …
})?
```

Additionally, `ColumnsBuilder` and `ScatterColumnsBuilder` have been unified into a single `OutputBuilder` type.

### `EntityPathFilter` removed from `Lens`

Entity path filtering is no longer part of the `Lens` itself. A `Lens` now operates purely
on component columns within a chunk. Entity path filtering is a separate concern handled
either upstream (e.g. via stream filtering) or at the `Lenses` collection level.

```rust
// Before
Lens::for_input_column(EntityPathFilter::all(), "component")
    .output_columns(|out| { /* … */ })?
    .build()

// After
Lens::for_input_column("component")
    .output_columns(|out| { /* … */ })?
    .build()
```

If you need entity path filtering, use `Lenses::add_lens_with_filter`:

```rust
let lenses = Lenses::new(OutputMode::DropUnmatched)
    .add_lens_with_filter(EntityPathFilter::parse_forgiving("sensors/**"), lens);
```

## `rerun rrd compact` renamed to `rerun rrd optimize`

`rerun rrd compact` is now `rerun rrd optimize`.

## URDF importer transform entity

The [URDF importer](../../howto/logging-and-ingestion/urdf.md) now loads the static transforms of the model to the `/tf_static` entity by default.
This replaces the model-dependent entity path of previous versions, and improves consistency with ROS data.

A custom entity path can be now also configured in the `UrdfTree` API in Python and Rust, if desired.
