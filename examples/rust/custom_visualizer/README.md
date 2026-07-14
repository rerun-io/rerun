<!--[metadata]
title = "Custom visualizer: HeightField"
thumbnail = "https://static.rerun.io/custom_visualizer/80562de16618a7f9f4e35fd9502ae61d7bb1d187/480w.png"
thumbnail_dimensions = [480, 354]
-->

<picture>
  <img src="https://static.rerun.io/custom_visualizer/80562de16618a7f9f4e35fd9502ae61d7bb1d187/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_visualizer/80562de16618a7f9f4e35fd9502ae61d7bb1d187/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_visualizer/80562de16618a7f9f4e35fd9502ae61d7bb1d187/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_visualizer/80562de16618a7f9f4e35fd9502ae61d7bb1d187/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_visualizer/80562de16618a7f9f4e35fd9502ae61d7bb1d187/1200w.png">
</picture>

Example showing how to add a custom visualizer with a custom GPU renderer to the Rerun Viewer.

A custom visualizer has three parts:

1. **A custom archetype** ([`height_field_archetype.rs`](./src/height_field_archetype.rs)) defines what data your visualizer operates on. In this example, a `HeightField` archetype implements the [`Archetype`](../../../crates/store/re_types_core/src/archetype.rs) trait and stores a 2D grid of height values as an image buffer with an optional colormap. Archetypes are collections of components that can be logged via the SDK and queried by the viewer.

2. **A visualizer system** ([`height_field_visualizer.rs`](./src/height_field_visualizer.rs)) implements the [`VisualizerSystem`](../../../crates/viewer/re_viewer_context/src/view/visualizer_system.rs) trait. Each frame, the viewer calls `execute`, which queries the data store for entities matching the archetype and produces draw data for the renderer. This is where you go from raw logged data to something renderable.

3. **A custom renderer** ([`height_field_renderer.rs`](./src/height_field_renderer.rs) + [`height_field.wgsl`](./shader/height_field.wgsl)) implements the [`Renderer`](../../../crates/viewer/re_renderer/src/renderer/mod.rs) trait and handles the GPU side: uploading data to textures and buffers, managing bind groups, and issuing draw calls. The WGSL shader generates the mesh procedurally from a height texture, computes normals via finite differences, and applies a colormap.

The visualizer is registered with the built-in [`Spatial3DView`](../../../crates/store/re_sdk_types/src/blueprint/views/spatial3d_view.rs), so it participates in the existing 3D scene alongside built-in visualizers like points, meshes, and boxes.

The example comes with a built-in recording of animated terrain, and also starts an SDK server which the Python or Rust logging SDK can connect to.

## Usage

Start it with `cargo run -p custom_visualizer`.
