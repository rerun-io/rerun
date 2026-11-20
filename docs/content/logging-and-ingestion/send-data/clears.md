---
title: Clear out data using tombstones
order: 700
description: Use Clear to mark data as no longer valid after a point in time
---
In order to create coherent views of streaming data, the Rerun Viewer shows the latest values for each visible entity at the current timepoint. But some data may not be valid for the entire recording even if there are no updated values. This page shows how to tell Rerun that something you've logged should no longer be shown.

## Log entities as cleared
The most straight forward option is to explicitly log that an entity has been cleared. Rerun allows you to do this by logging a special `Clear` to any path. The timepoint at which the `Clear` is logged is the time point after which that entity will no longer be visible in your views.

For example, if you have an object tracking application, your code might look something like this:

snippet: howto/clears[tracker]

## Clarify data meaning
In some cases, the best approach may be to rethink how you log data to better express what is actually happening. Take the following example where update frequencies don't match:

snippet: howto/clears[mismatched_frequency]

You could fix this example by logging `rr.Clear`, but in this case it makes more sense to change what you log to better express what is happening. Re-logging the image to another namespace on only the frames where the detection runs makes it explicit which frame was used as the input to the detector. This will create a second view in the Viewer that always allows you to see the frame that was used for the current detection input.

Here is an example fix, where `DetectorModel` stands in for your detection model:

snippet: howto/clears[detector_fix]

## Log data with spans instead of timepoints
In some cases you already know how long a piece of data will be valid at the time of logging.
Rerun does **not support** associating logged data with spans like `(from_timepoint, to_timepoint)` or `(timepoint, time-to-live)` ([#3008](https://github.com/rerun-io/rerun/issues/3008)).

### Workaround by manually clearing entities
The workaround is to manually clear data when it is no longer valid.

snippet: howto/clears[manual_clear]

## Deleting data

A `Clear` does not delete anything.
It is a tombstone: new data that hides older data from that time on.
The cleared data stays in the recording, can still be seen at earlier times, and still uses memory.

There is no API to delete the data of a single entity from a recording.
Instead, you can:

- **Close a whole recording** in the Viewer, from the Sources panel.
  From code, use the experimental `rerun.experimental.ViewerClient.close(…)` on a running Viewer, e.g. `ViewerClient.connect().close("current")`.
- **Drop entities from a file** with the Rerun CLI:

  ```sh
  rerun rrd filter --drop-entity /camera/raw input.rrd > output.rrd
  ```

- **Limit memory use** of the Viewer with `--memory-limit`: when the limit is reached, the Viewer drops the oldest data.
