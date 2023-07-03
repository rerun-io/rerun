---
title: Custom Space View UI
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/custom_space_view/src/main.rs
thumbnail: https://static.rerun.io/0ddce48924e92e4509c4caea3266d414ad76d961_custom_space_view_480w.jpeg
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/0ddce48924e92e4509c4caea3266d414ad76d961_custom_space_view_480w.jpeg">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/07fc2ec0fb45bd282cd942021bec82a8bf22929d_custom_space_view_768w.jpeg">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/2411b2c296230079e33bf075020510f10ccf086f_custom_space_view_1024w.jpeg">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/44ad47af559fbdd45aab5a663992f31e80876793_custom_space_view_1200w.jpeg">
  <img src="https://static.rerun.io/dc8cfa50e309ba2e2cd2b7647391cd74b7a0477f_api_demo_full.jpeg" alt="Custom Space View UI example screenshot">
</picture>

Example showing how to add custom Space View classes to extend the Rerun Viewer.

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.


[#2337](https://github.com/rerun-io/rerun/issues/2337): Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Testing it
Start it with `cargo run -p custom_space_view`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
