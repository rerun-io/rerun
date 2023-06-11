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

![The Rerun Viewer, extended with a custom Space View that is shown three times, each time showing points on a colored plane](https://static.rerun.io/9c04c0140552ff9ddd526f98381765382a71e86c_custom_space_view.jpeg)

Above screenshot shows the [`custom_space_view`](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_space_view) example.
This example demonstrates how to add a fully custom Space View class to Rerun on startup.
Space Views that are added this way have access to the exact same interfaces as all other Space Views,
meaning that any of the built-in Space Views serves can serve as an additional example on how to implement Space Views.

**⚠️ Note that the interface for adding Space Views are very far from stable.** Expect code implementing custom Space Views to break with every release of Rerun.

# Future work
We plan to also support embedding your own GUI widgets inside existing space views.
