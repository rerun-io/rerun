---
title: Log to several recordings from one process
order: 600
description: Create more than one recording from a single process, and why calling rr.init again replaces the global recording
---

`rr.init` creates one _global_ recording.
Every call to `rr.init` replaces that global recording, so you cannot use it to log to two recordings at the same time.

To log to several recordings from one process, create one `RecordingStream` per recording.

## Create one `RecordingStream` per recording

Each `RecordingStream` has its own application ID, recording ID, and sinks.
Give each stream a distinct recording ID, otherwise their data ends up in the [same recording](../data-model/recordings.md).

```python
import uuid

import rerun as rr

rec_a = rr.RecordingStream("rerun_example_several_recordings", recording_id=uuid.uuid4())
rec_b = rr.RecordingStream("rerun_example_several_recordings", recording_id=uuid.uuid4())

rec_a.spawn()
rec_b.connect_grpc()  # The Viewer started by `rec_a.spawn()` is now running.

rec_a.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
rec_b.log("points", rr.Points3D([[2, 2, 2], [3, 3, 3]]))
```

There are three ways to choose which recording a call goes to:

- Call the method on the stream: `rec_a.log(…)`, `rec_a.set_time(…)`, `rec_a.save(…)`, and so on.
  Every function that takes an optional `recording=` argument is also a method on `RecordingStream`.
- Pass the stream explicitly: `rr.log("points", rr.Points3D(…), recording=rec_a)`.
- Use the stream as a context manager.
  Inside the `with` block, `rr.log` and the other module-level functions use that stream on the current thread:

  ```python
  with rec_a:
      rr.log("points", rr.Points3D([[0, 0, 0]]))
  ```

## Change the default recording

The module-level functions such as `rr.log` look up the active recording in this order: the thread-local recording first, then the global recording.

- `rr.set_global_data_recording(rec)` makes `rec` the default for all threads.
- `rr.set_thread_local_data_recording(rec)` makes `rec` the default for the current thread only.
  Use this to give each worker thread its own recording.

Both functions return the recording they replaced.
You can also pass `make_default=True` or `make_thread_default=True` to the `RecordingStream` constructor.

## Rust and C++

In Rust, every `RecordingStream` is already explicit: create one per recording with `rerun::RecordingStreamBuilder::new(app_id).recording_id(…)`.

In C++, create one `rerun::RecordingStream(app_id, recording_id)` per recording.
Use `set_global()` or `set_thread_local()` to make one of them the default.

## Examples

- [Multithreading](https://github.com/rerun-io/rerun/blob/latest/examples/python/multithreading/multithreading.py): several threads logging to one recording.
- [Multiprocess logging](https://github.com/rerun-io/rerun/blob/latest/examples/python/multiprocess_logging/multiprocess_logging.py): several processes logging to one recording.

See also:
- [Share recordings across multiple processes](shared-recordings.md)
- [Recordings](../data-model/recordings.md)
- [🐍 Python `RecordingStream` API](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.RecordingStream)
