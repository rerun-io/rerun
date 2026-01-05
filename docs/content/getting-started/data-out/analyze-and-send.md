---
title: Analyze the data and send back the results
order: 3
---

In the previous sections, we explored our data and exported it to a Pandas dataframe. In this section, we will analyze the data to extract a "jaw open state" signal and send it back to the viewer.

## Analyze the data

We already identified that thresholding the `jawOpen` signal at 0.15 is all we need to produce a binary "jaw open state" signal.

In the [previous section](export-dataframe.md#inspect-the-dataframe), we prepared a flat, floating point column with the signal of interest called `"jawOpen"`. Let's add a boolean column to our Pandas dataframe to hold our jaw open state:

snippet: tutorials/data_out[filter_jaw]

## Send the result back to the viewer

The first step is to initialize the logging SDK targeting the same recording we just analyzed.
This requires using the exact same application ID and recording ID.
By using the same identifiers, we're appending new data to an existing recording.
If the recording is currently open in the viewer and listening for new connections, the new data will appear in the active session in real time.

snippet: tutorials/data_out[connect_viewer]

<!-- TODO(nick): mention layers? -->

_Note_: When automating data analysis, it is typically preferable to log the results to an distinct RRD file next to the source RRD (using `rr.save()`). In such a situation, it is also valid to use the same app ID and recording ID. This allows opening both the source and result RRDs in the viewer, which will display data from both files under the same recording.

We will send our jaw open state data in two forms:

1. As a standalone [`Scalar`](../../reference/types/components/scalar.md) component, to hold the raw data.
2. As a [`Text`](../../reference/types/components/text.md) component on the existing bounding box entity, such that we obtain a textual representation of the state in the visualization.

Here is how to send the data as a scalar:

snippet: tutorials/data_out[send_columns]

We use the [`rr.send_column()`](../../howto/send_columns.md) API to efficiently send the entire column of data in a single batch.

Next, let's send the same data as `Text` component:

snippet: tutorials/data_out[log_labels]

Here we first log the [`ShowLabel`](../../reference/types/components/show_labels.md) component as static to enable the display of the label. Then, we use `rr.send_column()` again to send an entire batch of text labels. We use [`np.where()`](https://numpy.org/doc/stable/reference/generated/numpy.where.html) to produce a label matching the state for each timestamp.

### Final result

With some adjustments to the viewer blueprint, we obtain the following result:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/getting-started-data-out/data-out-final-vp8.webm" type="video/webm" />
</video>

The OPEN/CLOSE label is displayed along the bounding box on the 2D view, and the `/jaw_open_state` signal is visible in both the timeseries and dataframe views.

### Complete script

Here is the complete script used by this guide to load data, analyze it, and send the result back:

snippet: tutorials/data_out
