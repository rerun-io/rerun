---
title: Share a recording across multiple processes
order: 2
---

A common need is to log data from multiple processes  and then visualize all of that data as part of a single shared recording.

Rerun has the notion of a [Recording ID](../concepts/apps-and-recordings) for that: any recorded datasets that share the same Recording ID will be visualized as one shared dataset.

The data can be logged from any number of processes, whether they run on the same machine or not, or implemented in different programming languages.  
All that matter is that they share the same Recording ID.

By default, Rerun generates a random Recording ID everytime you start a new logging session, but you can override that behavior, e.g.:

code-example: custom-recording-id

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

For more information, check out our [dedicated example](https://github.com/rerun-io/rerun/tree/main/examples/python/shared_recording?speculative-link).

### Caveats

We do not yet provide a way to merge [multiple recording files](https://github.com/rerun-io/rerun/issues/4057) into a single one directly from the CLI, although you can load all of them in the Rerun Viewer first and then use the save feature ([which has its own issues](https://github.com/rerun-io/rerun/issues/3091)).
