# Rerun changelog

A rough time-line of major user-facing things added, removed and changed. Newest on top.

* 2022-10-14: Add support for logging 3D Arrows ([#199](https://github.com/rerun-io/rerun/pull/199)).
* 2022-10-10: Python SDK: add `set_visible` API ([#176](https://github.com/rerun-io/rerun/pull/176)).
* 2022-10-07: Implement zooming and panning of 2D view ([#160](https://github.com/rerun-io/rerun/pull/160)).
* 2022-10-06: Support labels for 3D bounding boxes ([#159](https://github.com/rerun-io/rerun/pull/159)).
* 2022-10-06: Implement text entries ("logging logs") ([#153](https://github.com/rerun-io/rerun/pull/153), [#167](https://github.com/rerun-io/rerun/pull/167))
* 2022-10-06: Improve rendering of bounding box labels for small bounding boxes ([#157](https://github.com/rerun-io/rerun/pull/157)).
* 2022-10-06: Fix bug object visibility toggling ([#155](https://github.com/rerun-io/rerun/pull/155)).
* 2022-10-04: Update pinned rust version to 1.64 and use workspace inheritance ([#110](https://github.com/rerun-io/rerun/pull/110)).
* 2022-09-21: Python SDK: add `log_point` ([#106](https://github.com/rerun-io/rerun/pull/106)).
* 2022-09-21: Python SDK: add `log_obb` ([#103](https://github.com/rerun-io/rerun/pull/103)).
* 2022-09-20: Reduce memory use for image intensive applications ([#100](https://github.com/rerun-io/rerun/pull/100)).
* 2022-09-17: Add 'timeless' data ([#96](https://github.com/rerun-io/rerun/pull/96)).
* 2022-09-17: Time selection will now also include the latest data before the selection ([#98](https://github.com/rerun-io/rerun/pull/98)).
* 2022-09-10: Fix toggling visibility of point clouds ([#88](https://github.com/rerun-io/rerun/pull/88)).
* 2022-09-07: Python SDK: add `log_rects` ([#86](https://github.com/rerun-io/rerun/pull/86)).
* 2022-09-07: Python SDK: `log_rect` has changed signature to allow you to select your preferred rectangle format ([#85](https://github.com/rerun-io/rerun/pull/85)).
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
* 2022-05-09: Hover image in selection panel to zoom in on individual pixels.
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
