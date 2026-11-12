---
title: Share recordings across multiple processes
order: 500
description: Log to one recording from many processes, machines, or languages
---

A common need is to log data from multiple processes and then visualize all of that data as part of a single shared recording.

Rerun identifies a recording by its [application ID and recording ID](../data-model/recordings.md).
Any data that shares the same application ID _and_ the same recording ID is shown as one shared recording.
If either of them differs, you get two separate recordings.

The data can be logged from any number of processes, whether they run on the same machine or not, or implemented in different programming languages.
All that matters is that they use the same application ID and the same recording ID.

By default, Rerun generates a random recording ID every time you start a new logging session, but you can override that behavior, e.g.:

snippet: tutorials/custom-recording-id

It's up to you to decide where each recording ends up:
- all processes could stream their share of the data in real-time to a Rerun Viewer,
- or maybe they all write to their own file on disk that are later loaded in a viewer,
- or some other combination of the above.

Here's a simple example of such a workflow:
```sh
# Process 1 logs some spheres to a recording file.
./app1.py  # rr.init("rerun_example_shared", recording_id="my_shared_recording"); rr.save("/tmp/recording1.rrd")

# Process 2 logs some cubes to another recording file.
./app2.py  # rr.init("rerun_example_shared", recording_id="my_shared_recording"); rr.save("/tmp/recording2.rrd")

# Visualize a 3D scene with both spheres and cubes.
rerun /tmp/recording*.rrd  # they share the same application ID and recording ID!
```

For more information, check out our dedicated examples:
* [🐍 Python](https://github.com/rerun-io/rerun/blob/latest/examples/python/shared_recording/shared_recording.py)
* [🦀 Rust](https://github.com/rerun-io/rerun/blob/latest/examples/rust/shared_recording/src/main.rs)
* [🌊 C++](https://github.com/rerun-io/rerun/blob/latest/examples/cpp/shared_recording/main.cpp)


## Multiple threads

A `RecordingStream` can be shared between threads.
In Python, call `rr.init` once in the main thread, then call `rr.log` from any thread: all threads log to the same global recording.
See the [multithreading example](https://github.com/rerun-io/rerun/blob/latest/examples/python/multithreading/multithreading.py).

If different threads should log to different recordings, give each thread its own stream with `rr.set_thread_local_data_recording`, or use the stream as a context manager (`with rec: …`).
See [Log to several recordings from one process](multiple-recordings.md).

## Multiple Python processes (`multiprocessing`)

Each process must call `rr.init` itself, with the same application ID, and then connect to the Viewer.
Processes started with Python's `multiprocessing` inherit the default recording ID of the parent process.
If you set `recording_id` yourself in the parent, pass the same `recording_id` in every child.

```python
import multiprocessing

import rerun as rr


@rr.shutdown_at_exit  # Forked processes skip the `atexit` handler that flushes data.
def task(index: int) -> None:
    rr.init("rerun_example_multiprocessing")  # Same application ID as the parent.
    rr.connect_grpc()
    rr.log(f"task_{index}", rr.TextLog(f"hello from task {index}"))


if __name__ == "__main__":
    rr.init("rerun_example_multiprocessing")
    rr.spawn(connect=False)  # Start a Viewer that the child processes connect to.

    for i in range(3):
        p = multiprocessing.Process(target=task, args=(i,))
        p.start()
        p.join()
```

See the [multiprocess logging example](https://github.com/rerun-io/rerun/blob/latest/examples/python/multiprocess_logging/multiprocess_logging.py).

## Combining recording files with the Rerun CLI

To combine several `.rrd` files into one file, use the [Rerun CLI](../../reference/cli.md).

`rerun rrd merge` writes all inputs into one file, but keeps each recording separate: data is only combined if it already has the same application ID and recording ID.

```sh
rerun rrd merge -o merged.rrd first.rrd second.rrd
```

To force several files into one recording, use `rerun rrd route` to set the application ID and recording ID of all the data:

```sh
rerun rrd route --application-id my_app --recording-id my_recording -o combined.rrd first.rrd second.rrd
```

The Rerun CLI offers several options to manipulate recordings in different ways, check out [the CLI reference](../../reference/cli.md) for more information.
