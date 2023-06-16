---
title: Extend Viewer UI
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/extend_viewer_ui/src/main.rs
thumbnail: https://static.rerun.io/0fc466bbbeab3b7c4b5aaa283dd751cecf212acd_extend_viewer_ui_480w.png
---

Example showing how to wrap the Rerun Viewer in your own GUI.

You create your own [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) app and write your own GUI using [`egui`](https://github.com/emilk/egui).

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/0fc466bbbeab3b7c4b5aaa283dd751cecf212acd_extend_viewer_ui_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/58c3690ac06b3d958b168cd309502af094c323d0_extend_viewer_ui_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/bf522462a40c17c192860c57aa2ec61a6e7e527b_extend_viewer_ui_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/beefd70f69659298482a7c6536f54bb75c8f4e91_extend_viewer_ui_1200w.png">
  <img src="https://static.rerun.io/58a4dc5eb65fda147ce12b6bf35a710ae74d3050_extend_viewer_ui_full.png" alt="Extend Viewer UI example screenshot">
</picture>

[#2337](https://github.com/rerun-io/rerun/issues/2337): Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Testing it
Start it with `cargo run -p extend_viewer_ui`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
