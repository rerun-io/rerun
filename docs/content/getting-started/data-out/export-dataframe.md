---
title: Export the dataframe
order: 2
---


In the [previous section](explore-as-dataframe.md), we explored some face tracking data using the dataframe view. In this section, we will see how we can use the dataframe API of the Rerun SDK to export the data into a [Pandas](https://pandas.pydata.org) dataframe to further inspect and process it. 

## Load the recording

The dataframe SDK loads data from an .RRD file. The first step is thus to save the recording the viewer as RRD, which can be done from the Rerun menu:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/save_recording/ece0f887428b1800a305a3e30faeb57fa3d77cd8/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/save_recording/ece0f887428b1800a305a3e30faeb57fa3d77cd8/480w.png">
</picture>

We can then load the recording in a Python script as follows:

```python
import rerun as rr

# load the recording
recording = rr.dataframe.load_recording("face_tracking.rrd")
```


## Query the data

Once we have loaded a recording, we can query it to extract some data. Here is how it is done:

```python
# query the recording into a pandas dataframe
view = recording.view(index="frame_nr", contents="/blendshapes/0/jawOpen")
table = view.select().read_all()
```

A lot is happening here, let's go step by step:
1. We first create a _view_ into the recording. The view specifies which index column we want to use (in this case the `"frame_nr"` timeline), and which other content we want to consider (here, only the `/blendshapes/0/jawOpen` entity). The view defines a subset of all the data contained in the recording where each row has a unique value for the index, and columns are filtered based on the value(s) provided as `contents` argument.
2. A view can then be queried. Here we use the simplest possible form of querying by calling `select()`. No filtering is applied, and all view columns are selected. The result thus corresponds to the entire view.
3. The object returned by `select()` is a [`pyarrow.RecordBatchReader`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html). This is essentially an iterator that returns the stream of [`pyarrow.RecordBatch`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatch.html#pyarrow-recordbatch)es containing the query data.
4. Finally, we use the [`pyarrow.RecordBatchReader.read_all()`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html#pyarrow.RecordBatchReader.read_all) function to read all record batches as a [`pyarrow.Table`](https://arrow.apache.org/docs/python/generated/pyarrow.Table.html#pyarrow.Table).

**Note**: queries can obviously further narrow the returned data by filtering rows and/or selecting a subset of the view columns. See the documentation (TODO: LINK) for more information.

Let's have a look at the resulting table:

```python
print(table)
```

Here is the result:
```
pyarrow.Table
frame_nr: int64
frame_time: timestamp[ns]
log_tick: int64
log_time: timestamp[ns]
/blendshapes/0/jawOpen:Scalar: list<item: double>
  child 0, item: double
----
frame_nr: [[0],[1],...,[780],[781]]
frame_time: [[1970-01-01 00:00:00.000000000],[1970-01-01 00:00:00.040000000],...,[1970-01-01 00:00:31.200000000],[1970-01-01 00:00:31.240000000]]
log_tick: [[34],[67],...,[39252],[39310]]
log_time: [[2024-10-08 08:56:59.809678000],[2024-10-08 08:56:59.830410000],...,[2024-10-08 08:57:27.850207000],[2024-10-08 08:57:27.882733000]]
/blendshapes/0/jawOpen:Scalar: [[[0.00025173681206069887]],[[]],...,[[0.013143265619874]],[[0.01528632827103138]]]
```

TODO: update with actual recording

Again, this is a [PyArrow](https://arrow.apache.org/docs/python/index.html) table which contains the result of our query. Exploring this further is beyond the scope of the present guide. Yet, it is a reminder that Rerun natively stores—and returns—data in arrow format. As such, it efficiently interoperates with other Arrow-native and/or compatible tools such as Polars or DuckDB. 


## Create a Pandas dataframe

For this guide, we will use Pandas to 
Before exploring the data further, let's convert the table to a Pandas dataframe:

```python
df = table.to_pandas()
```

Alternatively, the dataframe can be created directly, without using the intermediate PyArrow table:

```python
df = view.select().read_pandas()
```


## Inspect the dataframe

Let's have a first look at this dataframe:

```python
print(df)
```

Here is the result:

```
     frame_nr              frame_time  log_tick                   log_time /blendshapes/0/jawOpen:Scalar
0           0 1970-01-01 00:00:00.000        34 2024-10-08 08:56:59.809678      [0.00025173681206069887]
1           1 1970-01-01 00:00:00.040        67 2024-10-08 08:56:59.830410                            []
2           2 1970-01-01 00:00:00.080        99 2024-10-08 08:56:59.869166        [0.006588249001652002]
3           3 1970-01-01 00:00:00.120       132 2024-10-08 08:56:59.897281                            []
4           4 1970-01-01 00:00:00.160       164 2024-10-08 08:56:59.938856       [0.0010859279427677393]
..        ...                     ...       ...                        ...                           ...
777       777 1970-01-01 00:00:31.080     39078 2024-10-08 08:57:27.748328        [0.010198011063039303]
778       778 1970-01-01 00:00:31.120     39136 2024-10-08 08:57:27.781440        [0.011381848715245724]
779       779 1970-01-01 00:00:31.160     39194 2024-10-08 08:57:27.815674        [0.011795849539339542]
780       780 1970-01-01 00:00:31.200     39252 2024-10-08 08:57:27.850207           [0.013143265619874]
781       781 1970-01-01 00:00:31.240     39310 2024-10-08 08:57:27.882733         [0.01528632827103138]

[782 rows x 5 columns]

```

TODO: update with actual recording + find empty places latter

We can make several observations from this output.

- The first four columns are timeline columns. These are the various timelines the data is logged to in this recording. 
- The last columns is named `/blendshapes/0/jawOpen:Scalar`. This is what we call a _component column_, and it corresponds to the [Scalar](../../reference/types/components/scalar.md) component logged to the `/blendshapes/0/jawOpen` entity.
- Each row in the `/blendshapes/0/jawOpen:Scalar` column consists of a list of scalars, in this case containing either zero or one values.

This last point may come as a surprise but is a consequence of Rerun's data model where components are always stored as arrays. This enables, for example, to log an entire point cloud using the [`Points3D`](../../reference/types/archetypes/points3d.md) archetype under a single entity and at a single timestamp.

Also, when entities are cleared (which happens by logging the special [`Clear`](../../reference/types/archetypes/clear.md) archetype), the dataframe API returns empty arrays for the corresponding timestamps—and all further timestamps until a new value is logged. This explains why some rows in our dataframe contain an empty list.

For scalar data, however, a flat representation using floating point numbers (using NaN for missing values) would be more convenient. This is achieved using the `explode()` function:

```python
df["jawOpen"] = df["/blendshapes/0/jawOpen:Scalar"].explode().astype(float)
print(df["jawOpen"])
```
Here is the result:
```
0      0.000252
1           NaN
2      0.006588
3           NaN
4      0.001086
         ...   
777    0.010198
778    0.011382
779    0.011796
780    0.013143
781    0.015286
Name: jawOpen, Length: 782, dtype: float64
```

TODO: add note on `dropna`

With this, we are ready for the [next section](analyze-and-log.md), where we will analyze the data and log back the result to the Rerun viewer.