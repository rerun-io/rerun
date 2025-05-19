---
title: Implement custom visualizations (Rust only)
order: 200
description: How to extend the Rerun Viewer UI using Rust and egui
---

## Embedding custom UI in the Viewer

![The Rerun Viewer, extended with a custom panel to the right](https://github.com/rerun-io/rerun/assets/1148717/cbbad63e-9b18-4e54-bafe-b6ffd723f63e)

In the above screenshot you see the example [`extend_viewer_ui`](https://github.com/rerun-io/rerun/tree/main/examples/rust/extend_viewer_ui), which contains the Rerun Viewer to the left and a custom panel to the right. In this example the panel contains a hierarchial text view of the loaded data.

### How to build it

The Rerun Viewer is defined by the crate [`re_viewer`](https://github.com/rerun-io/rerun/tree/main/crates/viewer/re_viewer). It uses the popular Rust GUI library [`egui`](https://github.com/emilk/egui) (written by our CTO) and its framework [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe). To extend the UI you need to create your own `eframe` application and embed `re_viewer` inside of it. You can then use `egui` to add custom panels and windows.

The best way to get started is by reading [the source code of the `extend_viewer_ui` example](https://github.com/rerun-io/rerun/tree/main/examples/rust/extend_viewer_ui).


## Custom views classes

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/1200w.png">
  <img src="https://static.rerun.io/custom_view/61089d7178b3bd7f9e3de36ee9c00e5fdf1c6f76/full.png" alt="The Rerun Viewer, extended with a custom View that is shown three times, each time showing points on a colored plane">
</picture>

Above screenshot shows the [`custom_view`](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_view) example.
This example demonstrates how to add a fully custom View class to Rerun on startup.
Views that are added this way have access to the exact same interfaces as all other Views,
meaning that any of the built-in Views serves can serve as an additional example on how to implement Views.

**⚠️ Note that the interface for adding Views are very far from stable.** Expect code implementing custom Views to break with every release of Rerun.

# Future work
We plan to also support embedding your own GUI widgets inside existing views.
