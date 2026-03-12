---
title: Migrating from 0.30 to 0.31
order: 979
---

## CLI

### MCAP "layers" renamed to "decoders"

The `-l` / `--layer` flag for `rerun mcap convert` has been renamed to `-d` / `--decoder`.

This change is motivated by the ambiguity of the term "layer", which is also a core concept of Rerun Cloud.

Before:

```bash
rerun mcap convert input.mcap -l protobuf -l stats -o output.rrd
rerun mcap convert input.mcap -l ros2msg -l raw -l recording_info -o output.rrd
```

After:

```bash
rerun mcap convert input.mcap -d protobuf -d stats -o output.rrd
rerun mcap convert input.mcap -d ros2msg -d raw -d recording_info -o output.rrd
```

When no `-d` flags are specified, all available decoders are still used by default (same behavior as before).

## Rust API

### `re_mcap`: Layer types renamed to Decoder

All public types in the `re_mcap` crate have been renamed from `Layer` to `Decoder`.

| Old name           | New name              |
|--------------------|-----------------------|
| `Layer`            | `Decoder`             |
| `MessageLayer`     | `MessageDecoder`      |
| `LayerIdentifier`  | `DecoderIdentifier`   |
| `LayerRegistry`    | `DecoderRegistry`     |
| `SelectedLayers`   | `SelectedDecoders`    |

The `layers` module has been renamed to `decoders` (e.g., `re_mcap::layers::McapRawLayer` is now `re_mcap::decoders::McapRawDecoder`).

### `McapLoader` API updated

`McapLoader::new()` now takes `SelectedDecoders` instead of `SelectedLayers`:

```rust
// Before
McapLoader::new(SelectedLayers::All)

// After
McapLoader::new(SelectedDecoders::All)
```
