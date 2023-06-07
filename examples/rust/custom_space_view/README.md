# Custom Space View UI
Example showing how to add custom Space View classes to extend the Rerun Viewer.

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.

TODO: Add image
![image](https://github.com/rerun-io/rerun/assets/1148717/cbbad63e-9b18-4e54-bafe-b6ffd723f63e)

Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Testing it
Start it with `cargo run -p custom_space_view`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
