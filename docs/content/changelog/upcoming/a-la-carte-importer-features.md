---
title: "A la carte features for file importers"
hidden: true
type: feature
---

### A la carte features for file importers

`re_importer` now gates every import format behind its own Cargo feature — `image`, `lerobot`, `mcap`, `parquet`, `urdf`, and `video` — and the `rerun` crate gates native video handling behind a new `video` feature.
All of them are on by default, so nothing changes unless you opt out.

Rust users who only need a few formats can now skip the rest and cut compile times.
For example, a project that only imports URDF:

```toml
re_importer = { version = "0.37", default-features = false, features = ["urdf"] }
```

Dropping the other importers cut roughly 20% off both debug and release build times in the author's project.

`re_sdk`'s `importers` feature still enables the full set, so `RecordingStream::log_file_from_path` keeps working with every format.
To benefit from the split, depend on `re_importer` directly and pick your formats there.
