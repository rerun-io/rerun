# Rerun changelog

A rough time-line of major user-facing things added, removed and changed. Newest on top.

* 2022-09-07: Python SDK: `log_rect` has changed signature to allow you to select your preferred rectangle format.
* 2022-09-06: Viewer: rearrange different space views by resizing and drag-dropping tabs ([#82](https://github.com/rerun-io/rerun/pull/82)).
* 2022-09-01: Python SDK: support logging from multiple processes ([#79](https://github.com/rerun-io/rerun/pull/79), [#80](https://github.com/rerun-io/rerun/pull/80)).
* 2022-09-01: Python SDK: add `rerun.save(…)` to save recorded data to file ([#78](https://github.com/rerun-io/rerun/pull/78)).
* 2022-08-30: The camera log type can link a 3D and 2D space ([#72](https://github.com/rerun-io/rerun/pull/72)).
* 2022-08-29: Python SDK: `rerun.log_camera` ([#68](https://github.com/rerun-io/rerun/pull/68)).
* 2022-08-25: Python SDK: `rerun.log_mesh_file` and `rerun.log_path` ([#59](https://github.com/rerun-io/rerun/pull/59)).
* 2022-08-24: Python SDK: `rerun.set_space_up` ([#56](https://github.com/rerun-io/rerun/pull/56)).
* 2022-08-23: Viewer: improve zoom-in view when hovering an image.
* 2022-08-18: Viewer: you can have multiple open recordings ([#37](https://github.com/rerun-io/rerun/pull/37)).
* 2022-08-17: Python SDK: add `rerun.set_time_…` functions.
* 2022-08-10: Viewer: don't pause or rewind when play reaches end of data.
* 2022-08-08: Add optional "label" to 2D bounding boxes.
* 2022-08-05: Generalize image as tensors ([#25](https://github.com/rerun-io/rerun/pull/25)).
* 2022-07-26: Add Python SDK.
* 2022-06-22: [Roll 3D view by dragging with right mouse button](https://github.com/rerun-io/rerun/commit/9db2a5ab49c136476b4252cf706d51d942c950f8).
* 2022-06-15: Add support for batch logging ([#13](https://github.com/rerun-io/rerun/pull/13)).
* 2022-05-12: Click on a point to center camera on that point in the 3D view.
* 2022-05-12: Use WSAD and QE to move camera in 3D view.
* 2022-05-09: Step forward/back in time with arrow keys.
* 2022-05-09: Hover image in context panel to zoom in on individual pixels.
* 2022-05-02: Improve the time ticks in the time panel.
* 2022-04-28: Follow camera in 3D by selecting a camera message or object path.
* 2022-04-26: Toggle visibility of object tree nodes on/off in time panel.
* 2022-04-26: 3D camera panning.
* 2022-04-26: Add `/3d_space_name/up` to specify the up axis.
* 2022-04-26: Add logging of 3D cameras.
* 2022-04-26: Misc optimizations.
* 2022-04-25: Logging of "raw" meshes.
* 2022-04-25: FPS setting in time panel when viewing sequences.
* 2022-04-25: 2D line segments.
* 2022-04-22: Support more image types.
* 2022-04-20: Time selection ([#4](https://github.com/rerun-io/rerun/pull/4)).
* 2022-04-19: Zoom and pan timeline ([#3](https://github.com/rerun-io/rerun/pull/3)).
* 2022-04-15: 2D path primitive.
* 2022-04-15: Save rerun data to file ([#2](https://github.com/rerun-io/rerun/pull/2)).
* 2022-04-13: Puffin profiler support ([#1](https://github.com/rerun-io/rerun/pull/1)).
* 2022-04-12: Save images to disk.
* 2022-04-11: Copy image to clipboard.
* 2022-04-08: Add button to reset app state.
* 2022-04-08: Initial commit to https://github.com/rerun-io/rerun (after around three weeks of development in an old repository).
