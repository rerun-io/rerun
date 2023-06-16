---
title: Extend the Viewer UI in Rust
order: 4
description: How to extend the Rerun Viewer UI using Rust and egui
---

# What you can build

## Custom UI embedding the Viewer

![The Rerun Viewer, extended with a custom panel to the right](https://github.com/rerun-io/rerun/assets/1148717/cbbad63e-9b18-4e54-bafe-b6ffd723f63e)

In the above screenshot you see the example [`extend_viewer_ui`](https://github.com/rerun-io/rerun/tree/main/examples/rust/extend_viewer_ui), which contains the Rerun Viewer to the left and a custom panel to the right. In this example the panel contains a hierarchial text view of the loaded data.

### How to build it

The Rerun Viewer is defined by the crate [`re_viewer`](https://github.com/rerun-io/rerun/tree/main/crates/re_viewer). It uses the popular Rust GUI library [`egui`](https://github.com/emilk/egui) (written by our CTO) and its framework [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe). To extend the UI you need to create your own `eframe` application and embed `re_viewer` inside of it. You can then use `egui` to add custom panels and windows.

The best way to get started is by reading [the source code of the `extend_viewer_ui` example](https://github.com/rerun-io/rerun/tree/main/examples/rust/extend_viewer_ui).


## Custom Space Views classes

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/0ddce48924e92e4509c4caea3266d414ad76d961_custom_space_view_480w.jpeg">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/07fc2ec0fb45bd282cd942021bec82a8bf22929d_custom_space_view_768w.jpeg">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/2411b2c296230079e33bf075020510f10ccf086f_custom_space_view_1024w.jpeg">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/44ad47af559fbdd45aab5a663992f31e80876793_custom_space_view_1200w.jpeg">
  <img src="https://static.rerun.io/dc8cfa50e309ba2e2cd2b7647391cd74b7a0477f_custom_space_view_full.jpeg" alt="The Rerun Viewer, extended with a custom Space View that is shown three times, each time showing points on a colored plane">
</picture>


Above screenshot shows the [`custom_space_view`](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_space_view) example.
This example demonstrates how to add a fully custom Space View class to Rerun on startup.
Space Views that are added this way have access to the exact same interfaces as all other Space Views,
meaning that any of the built-in Space Views serves can serve as an additional example on how to implement Space Views.

**⚠️ Note that the interface for adding Space Views are very far from stable.** Expect code implementing custom Space Views to break with every release of Rerun.

# Future work
We plan to also support embedding your own GUI widgets inside existing space views.
