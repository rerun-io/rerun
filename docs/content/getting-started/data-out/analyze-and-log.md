---
title: Analyze the data and log the results
order: 3
---



In the previous sections, we explored our data and exported it into a Pandas dataframe. In this section, we will analyze the data to extract a "jaw open state" signal and log it back to the viewer.



## Analyze the data

Well, this is not the most complicated part, as we already identified that thresholding the `jawOpen` signal at 0.15 is all we need. Recall that we already flattened that signal into a `"jawOpen"` dataframe column in the [previous section](export-dataframe.md#inspect-the-dataframe)

Let's add a boolean column to our Pandas dataframe to hold our jaw open state:

```python
df["jawOpenState"] = df["jawOpen"] > 0.15
```


## Log the result back to the viewer

The first step to log the data is to initialize the logging such that the data we log is routed to the exact same recording that we just analyzed. For this, both the application ID and the recording ID must match. Here is how it is done:

```python
rr.init(recording.application_id(), recording_id=recording.recording_id())
rr.connect()
```

Note: When automating data analysis, you should typically log the results to an RRD file distinct from the source RRD (using `rr.save()`). It is also valid to use the same app ID and recording ID in such a case. In particular, this allows opening both the source and result RRDs in the viewer, which will display both data under the same recording.

Let's log our jaw open state data in two forms:
1. As a standalone `Scalar` component, to hold the raw data.
2. As a `Text` component on the existing bounding box entity, such that we obtain a textual representation of the state in the visualization.

Here is how to log the data as a scalar:

```python
rr.send_columns(
    "/jaw_open_state",
    times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
    components=[
        rr.components.ScalarBatch(df["jawOpenState"]),
    ],
)
```

With use the [`rr.send_column()`](../../howto/send_columns.md) API to efficiently send the entire column of data in a single batch.

Next, let's log the same data as `Text` component:

```python
target_entity = "/video/detector/faces/0/bbox"
rr.log_components(target_entity, [rr.components.ShowLabels(True)], static=True)
rr.send_columns(
    target_entity,
    times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
    components=[
        rr.components.TextBatch(np.where(df["jawOpenState"], "OPEN", "CLOSE")),
    ],
)
```

Here we first log the [`ShowLabel`](../../reference/types/components/show_labels.md) component as static to enable the display of the label. Then, we use `rr.send_column()` again to send an entire batch of text labels. We use the [`np.where()`](https://numpy.org/doc/stable/reference/generated/numpy.where.html) to produce a label that matches the state for each timestamp.

### Final result

TODO: screen shot


### Complete script


snippet: tutorials/data_out