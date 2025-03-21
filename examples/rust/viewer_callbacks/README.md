<!--[metadata]
title = "Viewer callbacks"
thumbnail = "https://static.rerun.io/extend_viewer_ui/6ccfbe3718a50e659c484d31033db0bd9d40c262/480w.png"
thumbnail_dimensions = [480, 290]
-->


Example showing how to wrap the Rerun Viewer in your own GUI, and register callbacks to the Viewer.

You create your own [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) app and write your own GUI using [`egui`](https://github.com/emilk/egui).

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.

<picture>
  <img src="https://static.rerun.io/viewer_callbacks_example/3552bcd27112bb3889c7f0549e3fb96e0061c31c/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_callbacks_example/3552bcd27112bb3889c7f0549e3fb96e0061c31c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_callbacks_example/3552bcd27112bb3889c7f0549e3fb96e0061c31c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_callbacks_example/3552bcd27112bb3889c7f0549e3fb96e0061c31c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_callbacks_example/3552bcd27112bb3889c7f0549e3fb96e0061c31c/1200w.png">
</picture>

[#2337](https://github.com/rerun-io/rerun/issues/2337): Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Usage
Start it with `cargo run -p viewer_callbacks`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
