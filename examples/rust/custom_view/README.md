<!--[metadata]
title = "Custom view UI"
thumbnail = "https://static.rerun.io/custom_space_view/e05a073d64003645b6af6de91b068c2f646c1b8a/480w.jpeg"
thumbnail_dimensions = [480, 343]
-->


TODO: update screenshots
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/api_demo/e05a073d64003645b6af6de91b068c2f646c1b8a/480w.jpeg">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/api_demo/e05a073d64003645b6af6de91b068c2f646c1b8a/768w.jpeg">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/api_demo/e05a073d64003645b6af6de91b068c2f646c1b8a/1024w.jpeg">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/api_demo/e05a073d64003645b6af6de91b068c2f646c1b8a/1200w.jpeg">
  <img src="https://static.rerun.io/api_demo/e05a073d64003645b6af6de91b068c2f646c1b8a/full.jpeg" alt="Custom View UI example screenshot">
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
