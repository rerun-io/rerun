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

The Lenses API has been restructured and simplified.

### Entity path filtering moved to `Lenses`

Entity path filtering is no longer part of the `Lens` itself. Use
`Lenses::add_lens_with_filter`:

```rust
let lenses = Lenses::new(OutputMode::DropUnmatched)
    .add_lens_with_filter(EntityPathFilter::parse_forgiving("sensors/**"), lens);
```

This makes applying lenses to individual chunks more ergonomic.

### New builder API

Lenses are now created through `Lens::derive()`, `Lens::scatter()`, and `Lens::mutate()`:

```rust
// Before
Lens::for_input_column(EntityPathFilter::all(), "component")
    .output_columns(|out| { /* … */ })?
    .build()

// After - derive lens (1:1 row mapping)
Lens::derive("component")
    .to_component(component_descr, ".field")
    .build()?

// After - scatter lens (1:N row mapping)
Lens::scatter("component")
    .output_entity("/target")
    .to_component(component_descr, ".field")
    .build()?

// After - mutate lens (modifies component in-place)
Lens::mutate("component", ".field").build()
```

To output columns to multiple entities from a single component, multiple lenses can be registered for the same input component.

## `rerun rrd compact` renamed to `rerun rrd optimize`

`rerun rrd compact` is now `rerun rrd optimize`.

## URDF importer transform entity

The [URDF importer](../../howto/logging-and-ingestion/urdf.md) now loads the static transforms of the model to the `/tf_static` entity by default.
This replaces the model-dependent entity path of previous versions, and improves consistency with ROS data.

A custom entity path can be now also configured in the `UrdfTree` API in Python and Rust, if desired.
