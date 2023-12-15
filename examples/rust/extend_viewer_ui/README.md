---
title: Extend Viewer UI
thumbnail: https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/480w.png
---

Example showing how to wrap the Rerun Viewer in your own GUI.

You create your own [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) app and write your own GUI using [`egui`](https://github.com/emilk/egui).

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/1200w.png">
  <img src="https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/full.png" alt="Extend Viewer UI example screenshot">
</picture>

[#2337](https://github.com/rerun-io/rerun/issues/2337): Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Testing it
Start it with `cargo run -p extend_viewer_ui`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
