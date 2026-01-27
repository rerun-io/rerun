---
title: Using layers to append data to segments
order: 150
---



In the [catalog object model](../../concepts/query-and-transform/catalog-object-model.md), datasets are a collection of segments, which are a collection of layers identified by a name.
Layers are immutable, but data can be added to segments by registering other layers with the same recording id but a different layer name.
This how-to page provides examples for two ways data can be added to existing datasets through layers.

Note: layers should not be confused with [MCAP layers](../../concepts/logging-and-ingestion/mcap/layers-explained.md), which serve a different purpose in the context of MCAP file ingestion.


## Adding data to existing segments using layers

When registering recordings to a dataset, the recordings are assigned the `"base"` layer name by default.

Let's register a few recordings from the [DROID](https://droid-dataset.github.io/) dataset (included in the Rerun repository for testing) to illustrate this:

snippet: howto/layers[setup]

Output:

```
┌───────────────────────────────────────┬───────────────────┐
│ rerun_segment_id                      ┆ rerun_layer_names │
│ ---                                   ┆ ---               │
│ type: Utf8                            ┆ type: List[Utf8]  │
╞═══════════════════════════════════════╪═══════════════════╡
│ ILIAD_50aee79f_2023_07_12_20h_55m_08s ┆ [base]            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_20_10h_40m_10s ┆ [base]            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_28_11h_25m_26s ┆ [base]            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_j807b3f8_2023_06_15_13h_42m_56s ┆ [base]            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s ┆ [base]            │
└───────────────────────────────────────┴───────────────────┘
```

It is possible to append data to existing segments by creating new `.rrd` files with matching recording IDs, and registering them to the dataset under new layer names.
A common workflow is to query existing segment data, compute derived values (such as metrics or embeddings), and add them as new layers.

As an example, we use the dataset registered previously and compute the tracking error (L2 norm between commanded and actual joint positions) of the robotic arm:

snippet: howto/layers[add_tracking_error]

The key steps are:
1. Query action (commanded) and observation (actual) joint positions from the dataset
2. For each segment, compute the L2 norm of the difference as tracking error
3. Create a new `.rrd` file with the same `recording_id` as the original segment
4. Log the derived data using `send_columns()` for efficient columnar logging
5. Register all derived `.rrd` files to the dataset with a `"tracking_error"` layer name

The `"rerun_layer_names"` column of the segment table confirms the new layer was added:

snippet: howto/layers[check_layer_names]

Output:

```
┌───────────────────────────────────────┬────────────────────────┐
│ rerun_segment_id                      ┆ rerun_layer_names      │
│ ---                                   ┆ ---                    │
│ type: Utf8                            ┆ type: List[Utf8]       │
╞═══════════════════════════════════════╪════════════════════════╡
│ ILIAD_50aee79f_2023_07_12_20h_55m_08s ┆ [base, tracking_error] │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_20_10h_40m_10s ┆ [base, tracking_error] │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_28_11h_25m_26s ┆ [base, tracking_error] │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_j807b3f8_2023_06_15_13h_42m_56s ┆ [base, tracking_error] │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s ┆ [base, tracking_error] │
└───────────────────────────────────────┴────────────────────────┘
```

For another example of computing derived data, see the [Query video streams](../query-and-transform/query_videos.md) page.

In the next section, we will demonstrate that the data has indeed been added by querying the dataset again.

## Adding properties to segments using layers

In addition to regular Rerun data, layers can be used to add [properties](../../concepts/query-and-transform/properties-and-segments.md) to segments.
This is useful for tagging segments with derived metadata based on their content.

In this example, we query the tracking error computed in the previous section, calculate the mean error per segment, and create a `tracking_good` boolean property based on a threshold:

snippet: howto/layers[add_quality_property]

The key steps are:
1. Query the derived tracking error data we just added
2. Use DataFusion's `aggregate()` to compute the mean error per segment
3. Threshold the mean to create a boolean `tracking_good` property
4. Create new `.rrd` files with `send_property()` to log the property
5. Register under a separate `"quality"` layer

The property now appears in the segment table:

snippet: howto/layers[verify]


Output:

```
┌───────────────────────────────────────┬─────────────────────────────────┬────────────────────────────────────┐
│ rerun_segment_id                      ┆ rerun_layer_names               ┆ property:quality:tracking_good     │
│ ---                                   ┆ ---                             ┆ ---                                │
│ type: Utf8                            ┆ type: List[Utf8]                ┆ type: nullable List[nullable bool] │
│                                       ┆                                 ┆ component: tracking_good           │
│                                       ┆                                 ┆ entity_path: /__properties/quality │
│                                       ┆                                 ┆ kind: data                         │
╞═══════════════════════════════════════╪═════════════════════════════════╪════════════════════════════════════╡
│ ILIAD_50aee79f_2023_07_12_20h_55m_08s ┆ [base, tracking_error, quality] ┆ [false]                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_20_10h_40m_10s ┆ [base, tracking_error, quality] ┆ [false]                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_28_11h_25m_26s ┆ [base, tracking_error, quality] ┆ [true]                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_j807b3f8_2023_06_15_13h_42m_56s ┆ [base, tracking_error, quality] ┆ [true]                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_sbd7d2c6_2023_12_24_16h_20m_37s ┆ [base, tracking_error, quality] ┆ [true]                             │
└───────────────────────────────────────┴─────────────────────────────────┴────────────────────────────────────┘
```

See [this page](../../concepts/query-and-transform/properties-and-segments.md) for a deep dive into properties and segment tables.

## FAQ

### Can I modify a layer after it has been registered?

No.
Layers are immutable.

### What happens if I register to an existing layer name?

Registering a `.rrd` file with a `recording_id` and `layer_name` that already exists will result in an error.

### How can I replace an existing layer?

<!-- TODO(RR-3451) update this when the python API is updated -->

There is currently no way to replace an existing layer using the Python SDK.
The current workaround consists of recreating the dataset.

### Can I query a single layer with the dataframe query?

No.
Segments are considered an aggregation of all their layers.
There is currently no way to query data from a single layer only.

### Must layers be registered to all segments in a dataset?

No.
Each segment can have its own set of layers.
Some segments may have additional layers that others do not.

### What is the default layer name?

When you register a recording without specifying a `layer_name`, it is assigned to the `"base"` layer.


### Is it possible to obtain a dataframe with a list of all layers in a dataset?

Yes.
The [`DatasetEntry.manifest()`](https://ref.rerun.io/docs/python/stable/common/catalog/#rerun.catalog.DatasetEntry.manifest) method returns a DataFusion DataFrame containing the full dataset manifest, which includes layer information for each segment:

snippet: howto/layers[manifest]

Output:

```
┌───────────────────────────────────────┬──────────────────┬────────────────────────────────────┐
│ rerun_segment_id                      ┆ rerun_layer_name ┆ property:quality:tracking_good     │
│ ---                                   ┆ ---              ┆ ---                                │
│ type: Utf8                            ┆ type: Utf8       ┆ type: nullable List[nullable bool] │
│                                       ┆                  ┆ component: tracking_good           │
│                                       ┆                  ┆ entity_path: /__properties/quality │
│                                       ┆                  ┆ kind: data                         │
╞═══════════════════════════════════════╪══════════════════╪════════════════════════════════════╡
│ ILIAD_50aee79f_2023_07_12_20h_55m_08s ┆ base             ┆ null                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_50aee79f_2023_07_12_20h_55m_08s ┆ quality          ┆ [false]                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_50aee79f_2023_07_12_20h_55m_08s ┆ tracking_error   ┆ null                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_20_10h_40m_10s ┆ base             ┆ null                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_20_10h_40m_10s ┆ quality          ┆ [false]                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_20_10h_40m_10s ┆ tracking_error   ┆ null                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_28_11h_25m_26s ┆ base             ┆ null                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_28_11h_25m_26s ┆ quality          ┆ [true]                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_5e938e3b_2023_07_28_11h_25m_26s ┆ tracking_error   ┆ null                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ILIAD_j807b3f8_2023_06_15_13h_42m_56s ┆ base             ┆ null                               │
└───────────────────────────────────────┴──────────────────┴────────────────────────────────────┘
```
