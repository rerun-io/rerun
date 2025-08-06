<!--[metadata]
title = "Custom view UI"
thumbnail = "https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/480w.png"
thumbnail_dimensions = [480, 264]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/1200w.png">
  <img src="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/full.png" alt="Custom View UI example screenshot">
</picture>

Example showing how to add custom View classes to extend the Rerun Viewer.

The example is really basic, but should be something you can build upon.

The example starts an SDK server which the Python or Rust logging SDK can connect to.


[#2337](https://github.com/rerun-io/rerun/issues/2337): Note that in order to spawn a web viewer with these customizations applied,
you have to build the web viewer of the version yourself.
This is currently not supported outside of the Rerun repository.

## Testing it
Start it with `cargo run -p custom_view`.

Then put some data into it with: `cargo run -p minimal_options -- --connect`
