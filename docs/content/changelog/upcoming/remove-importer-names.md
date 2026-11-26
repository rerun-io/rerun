---
title: Removed deprecated data loader names and executable prefix
hidden: true
type: breaking
---

Rerun 0.44 removes the data loader aliases and executable prefix deprecated in 0.32.

Rename executables from `rerun-loader-*` to `rerun-importer-*` so the SDK can discover them.
Use `EXTERNAL_IMPORTER_INCOMPATIBLE_EXIT_CODE` instead of `EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE` in Rust, Python, and C++.
In C, replace the `rr_data_loader_settings` typedef with `rr_importer_settings`.
In Rust, replace `EXTERNAL_DATA_LOADER_PREFIX`, `DataLoader`, `DataLoaderError`, `DataLoaderSettings`, and `LoadedData` with `EXTERNAL_IMPORTER_PREFIX`, `Importer`, `ImporterError`, `ImporterSettings`, and `ImportedData`.
