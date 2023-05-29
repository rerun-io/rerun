# External Viewer UI
Example showing how to wrap the Rerun Viewer in your own GUI.

You create your own [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) app and write your own GUI using [`egui`](https://github.com/emilk/egui).

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.

![image](https://github.com/rerun-io/rerun/assets/1148717/cbbad63e-9b18-4e54-bafe-b6ffd723f63e)

## Testing it
Start it with `cargo run -p extend_viewer_ui`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
