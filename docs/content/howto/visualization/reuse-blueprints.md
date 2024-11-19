---
title: Re-use blueprints across sessions and SDKs
order: 150
---

While the [blueprint APIs](configure-viewer-through-code.md) are currently only available through [🐍 Python](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/), blueprints can be saved to file and then re-logged as needed from any language our SDKs support.

This enables you to re-use your saved blueprints both from any language we support as well as across different recordings that share a similar-enough structure, and makes it possible to share those blueprints with other users.

For this you'll need to create a blueprint file and _import_ that file when needed.


## Creating a blueprint file

Blueprint files (`.rbl`, by convention) can currently be created in two ways.

One is to use the Rerun viewer to interactively build the blueprint you want (e.g. by moving panels around, changing view settings, etc), and then using `Menu > Save blueprint` (or the equivalent palette command) to save the blueprint as a file.

The other is to use the [🐍 Python blueprint API](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) to programmatically build the blueprint, and then use the [`Blueprint.save`](https://ref.rerun.io/docs/python/0.19.0/common/blueprint_apis/#rerun.blueprint.Blueprint.save) method to save it as a file:

snippet: tutorials/visualization/save_blueprint


## (Re)Using a blueprint file

There are two ways to re-use a blueprint file.

The interactive way is to import the blueprint file directly into the Rerun viewer, using either `Menu > Import…` (or the equivalent palette command) or simply by drag-and-dropping the blueprint file into your recording.

The programmatic way works by calling `log_file_from_path`:
* [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
* [🦀 Rust `log_file_from_path`](https://ref.rerun.io/docs/rust/stable/rerun/struct.RecordingStream.html#method.log_file_from_path)
* [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)

This method allows you to log any file that contains data that Rerun understands (in this case, blueprint data) as part of your current recording:

snippet: tutorials/visualization/load_blueprint


## Limitation: dynamic blueprints

Sometimes, you might need your blueprint to dynamically react to the data you receive at runtime (e.g. you want to create one view per anomaly detected, and there is no way of knowing how many anomalies you're going to detect until the program actually runs).

The only way to deal with these situations today is to use the [🐍 Python](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) API.
