---
title: React to events in the Viewer
order: 400
---

We support registering callbacks to Viewer events in these environments:

- Web browsers, through our [JS package](../integrations/embed-web.md#callbacks)
- Jupyter notebooks, through our [Notebook API](../integrations/embed-notebooks.md#reacting-to-events-in-the-viewer)

For users extending the Viewer through the Rust [`re_viewer`](https://docs.rs/re_viewer/latest/re_viewer/) crate, there are two options:

- Use [`StartupOptions.on_event`](https://docs.rs/re_viewer/latest/re_viewer/struct.StartupOptions.html#structfield.on_event) to register
  the same events available on the web and in Jupyter.
- [Extend the UI](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_callback) to add your own widgets using `egui`, and
  fire completely custom events.
