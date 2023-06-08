# Custom Space View UI
Example showing how to add custom Space View classes to extend the Rerun Viewer.

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.

![image](https://static.rerun.io/9c04c0140552ff9ddd526f98381765382a71e86c_custom_space_view.jpeg)

[#2337](https://github.com/rerun-io/rerun/issues/2337): Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Testing it
Start it with `cargo run -p custom_space_view`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
