# Official Rerun examples

* [C++](cpp)
* [Python](python)
* [Rust](rust)

> Note: Make sure your SDK version matches the code in the examples.
For example, if your SDK version is `0.4.0`, check out the matching tag
for this repository by running `git checkout v0.4.0`.

## Documentation

The rendered examples documentation can be seen [here](https://rerun.io/examples).

The examples currently use the following structure:
```
examples/
  python/
    <name>/
      README.md
      main.py
      requirements.txt
  rust/
    <name>/
      README.md
      Cargo.toml
      src/
        main.rs
```

The important part is that each example has a `README.md` file. This file contains a brief description of the example, as well as installation/usage instructions. The `README.md` file also contains metadata in the form of frontmatter:
```
---
title: Text Logging
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/detect_and_track_objects/main.py
tags: [2D, huggingface, object-detection, object-tracking, opencv]
---

…
```

The contents of this `README.md` file and its frontmatter are used to render the examples in [the documentation](https://rerun.io/examples). Individual examples are currently "stitched together" to form one large markdown file for every category of examples (`artificial-data`, `real-data`).

The `manifest.yml` file describes the structure of the examples contained in this repository. Only the examples which appear in the manifest are included in the [generated documentation](https://rerun.io/examples). The file contains a description of its own format.

## Adding a new example

You can base your example off of `python/template` or `rust/template`.
Once it's ready to be displayed in the docs, add it to the [manifest](./manifest.yml).

If you want to run the example on CI and include it in the in-viewer example page,
add a `channel` entry to its README frontmatter. The available channels right now are:
- `main` for simple/fast examples built on each PR and the `main` branch
- `nightly` for heavier examples built once per day
