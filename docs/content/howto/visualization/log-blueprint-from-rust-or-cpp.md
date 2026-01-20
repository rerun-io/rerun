---
title: Log a blueprint from Rust or C++
order: 501
---

The blueprint API is currently only available in Python. However, blueprints can be saved to files and loaded from any language.

This enables a workflow where you:
1. Create and iterate on blueprints using Python (or the Viewer)
2. Save the blueprint to a `.rbl` file
3. Load that blueprint from Rust or C++ alongside your data


## Save a blueprint from Python

Use the Viewer's **Menu > Save blueprint** option, or save programmatically:

snippet: howto/visualization/save_blueprint


## Load the blueprint from Rust or C++

Use `log_file_from_path` to load the blueprint file as part of your recording:

snippet: howto/visualization/load_blueprint


## API reference

- [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
- [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
- [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)

You can learn more about saving and loading blueprints in [Configure the Viewer](../../getting-started/configure-the-viewer.md#save-and-load-blueprint-files).
