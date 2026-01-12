---
title: Share recordings across multiple processes
order: 400
---

A common need is to log data from multiple processes and then visualize all of that data as part of a single shared recording.

Rerun has the notion of a [Recording ID](../../concepts/logging-and-ingestion/apps-and-recordings.md) for that: any recorded datasets that share the same Recording ID will be visualized as one shared dataset.

The data can be logged from any number of processes, whether they run on the same machine or not, or implemented in different programming languages.
All that matter is that they share the same Recording ID.

By default, Rerun generates a random Recording ID everytime you start a new logging session, but you can override that behavior, e.g.:

snippet: tutorials/custom-recording-id

It's up to you to decide where each recording ends up:
- all processes could stream their share of the data in real-time to a Rerun Viewer,
- or maybe they all write to their own file on disk that are later loaded in a viewer,
- or some other combination of the above.

Here's a simple example of such a workflow:
```python
# Process 1 logs some spheres to a recording file.
./app1.py  # rr.init(recording_id='my_shared_recording', rr.save('/tmp/recording1.rrd')

# Process 2 logs some cubes to another recording file.
./app2.py  # rr.init(recording_id='my_shared_recording', rr.save('/tmp/recording2.rrd')

# Visualize a 3D scene with both spheres and cubes.
rerun /tmp/recording*.rrd  # they share the same Recording ID!
```

For more information, check out our dedicated examples:
* [🐍 Python](https://github.com/rerun-io/rerun/blob/latest/examples/python/shared_recording/shared_recording.py)
* [🦀 Rust](https://github.com/rerun-io/rerun/blob/latest/examples/rust/shared_recording/src/main.rs)
* [🌊 C++](https://github.com/rerun-io/rerun/blob/latest/examples/cpp/shared_recording/main.cpp)


### Merging recordings with the Rerun CLI

It is possible to merge multiple recording files into a single one using the [Rerun CLI](../../reference/cli.md#rerun-rrd-merge), e.g. `rerun rrd merge -o merged_recordings.rrd my_first_recording.rrd my_second_recording.rrd`.

The Rerun CLI offers several options to manipulate recordings in different ways, check out [the CLI reference](../../reference/cli.md) for more information.
