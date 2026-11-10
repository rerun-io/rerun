---
title: React to events in the Viewer
order: 400
description: Add buttons, sliders, or callbacks to the Viewer, and react to selection and time changes in web, Jupyter, or Rust
---

## Can I add buttons or sliders that call my Python script?

Not in the native Viewer.
The native Viewer has no user-defined widgets (buttons, sliders, text fields), and it does not send events (such as selection changes) back to a Python logging script.
See [missing features](../../development/roadmap.md#missing-features).

Here is what you can do instead.

## Callbacks in the web Viewer and in notebooks

We support registering callbacks to Viewer events, such as selection and time changes, in these environments:

- Web browsers, through our [JS package](../integrations/embed-web.md#callbacks)
- Jupyter notebooks, through our [Notebook API](../integrations/embed-notebooks.md#reacting-to-events-in-the-viewer)

In both cases, you can build your own widgets in the host page or notebook (for example with `ipywidgets`), and use them to control the Viewer.

## Callbacks in Rust

For users extending the Viewer through the Rust [`re_viewer`](https://docs.rs/re_viewer/latest/re_viewer/) crate, there are two options:

- Use [`StartupOptions.on_event`](https://docs.rs/re_viewer/latest/re_viewer/struct.StartupOptions.html#structfield.on_event) to register the same events available on the web and in Jupyter.
  The [`viewer_callbacks`](https://github.com/rerun-io/rerun/tree/latest/examples/rust/viewer_callbacks) example shows how.
- [Extend the UI](extend-ui.md) to add your own widgets using `egui`, and fire completely custom events.
  The [`custom_callback`](https://github.com/rerun-io/rerun/tree/latest/examples/rust/custom_callback) example shows how.

## Drive the Viewer from Python

The experimental `rerun.experimental.ViewerClient` connects to a running Viewer and controls it from Python.
It can open and close recordings, move the time cursor, start and stop playback, read and replace the blueprint, and take screenshots.
It can also read a snapshot of the Viewer state (open recordings, the time cursor, and the views), but not the current selection.
See [Control playback and drive the Viewer from code](control-the-viewer.md).
