---
title: Export the dataframe
order: 2
---


In the [previous section](explore-as-dataframe.md), we explored some face tracking data using the dataframe view. In this section, we will see how we can use the dataframe API of the Rerun SDK to export the same data into a [Pandas](https://pandas.pydata.org) dataframe to further inspect and process it.

## Load the recording

The dataframe SDK loads data from an .RRD file.
The first step is thus to save the recording as RRD, which can be done from the Rerun menu:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/save_recording/ece0f887428b1800a305a3e30faeb57fa3d77cd8/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/save_recording/ece0f887428b1800a305a3e30faeb57fa3d77cd8/480w.png">
</picture>

We can then load the recording in a Python script as follows:

```python
import rerun as rr
import numpy as np # We'll need this later.

# load the recording
recording = rr.dataframe.load_recording("face_tracking.rrd")
```


## Query the data

Once we loaded a recording, we can query it to extract some data. Here is how it is done:

```python
# query the recording into a pandas dataframe
view = recording.view(
    index="frame_nr",
    contents="/blendshapes/0/jawOpen"
)
table = view.select().read_all()
```

A lot is happening here, let's go step by step:
1. We first create a _view_ into the recording. The view specifies which index column we want to use (in this case the `"frame_nr"` timeline), and which other content we want to consider (here, only the `/blendshapes/0/jawOpen` entity). The view defines a subset of all the data contained in the recording where each row has a unique value for the index, and columns are filtered based on the value(s) provided as `contents` argument.
2. A view can then be queried. Here we use the simplest possible form of querying by calling `select()`. No filtering is applied, and all view columns are selected. The result thus corresponds to the entire view.
3. The object returned by `select()` is a [`pyarrow.RecordBatchReader`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html). This is essentially an iterator that returns the stream of [`pyarrow.RecordBatch`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatch.html#pyarrow-recordbatch)es containing the query data.
4. Finally, we use the [`pyarrow.RecordBatchReader.read_all()`](https://arrow.apache.org/docs/python/generated/pyarrow.RecordBatchReader.html#pyarrow.RecordBatchReader.read_all) function to read all record batches as a [`pyarrow.Table`](https://arrow.apache.org/docs/python/generated/pyarrow.Table.html#pyarrow.Table).

**Note**: queries can be further narrowed by filtering rows and/or selecting a subset of the view columns. See the reference documentation for more information.

<!-- TODO(#7499): add a link to the reference documentation -->

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
frame_nr: [[0],[1],...,[412],[413]]
frame_time: [[1970-01-01 00:00:00.000000000],[1970-01-01 00:00:00.040000000],...,[1970-01-01 00:00:16.480000000],[1970-01-01 00:00:16.520000000]]
log_tick: [[34],[92],...,[22077],[22135]]
log_time: [[2024-10-13 08:26:46.819571000],[2024-10-13 08:26:46.866358000],...,[2024-10-13 08:27:01.722971000],[2024-10-13 08:27:01.757358000]]
/blendshapes/0/jawOpen:Scalar: [[[0.03306490555405617]],[[0.03812221810221672]],...,[[0.06996039301156998]],[[0.07366073131561279]]]
```

Again, this is a [PyArrow](https://arrow.apache.org/docs/python/index.html) table which contains the result of our query. Further exploring Arrow structures is beyond the scope of this guide. Yet, it is a reminder that Rerun natively stores—and returns—data in arrow format. As such, it efficiently interoperates with other Arrow-native and/or compatible tools such as [Polars](https://pola.rs) or [DuckDB](https://duckdb.org).


## Create a Pandas dataframe

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

<!-- NOLINT_START -->

```
     frame_nr              frame_time  log_tick                   log_time /blendshapes/0/jawOpen:Scalar
0           0 1970-01-01 00:00:00.000        34 2024-10-13 08:26:46.819571         [0.03306490555405617]
1           1 1970-01-01 00:00:00.040        92 2024-10-13 08:26:46.866358         [0.03812221810221672]
2           2 1970-01-01 00:00:00.080       150 2024-10-13 08:26:46.899699        [0.027743922546505928]
3           3 1970-01-01 00:00:00.120       208 2024-10-13 08:26:46.934704        [0.024137917906045914]
4           4 1970-01-01 00:00:00.160       266 2024-10-13 08:26:46.967762        [0.022867577150464058]
..        ...                     ...       ...                        ...                           ...
409       409 1970-01-01 00:00:16.360     21903 2024-10-13 08:27:01.619732         [0.07283800840377808]
410       410 1970-01-01 00:00:16.400     21961 2024-10-13 08:27:01.656455         [0.07037288695573807]
411       411 1970-01-01 00:00:16.440     22019 2024-10-13 08:27:01.689784         [0.07556036114692688]
412       412 1970-01-01 00:00:16.480     22077 2024-10-13 08:27:01.722971         [0.06996039301156998]
413       413 1970-01-01 00:00:16.520     22135 2024-10-13 08:27:01.757358         [0.07366073131561279]

[414 rows x 5 columns]
```

<!-- NOLINT_END -->

We can make several observations from this output.

- The first four columns are timeline columns. These are the various timelines the data is logged to in this recording.
- The last columns is named `/blendshapes/0/jawOpen:Scalar`. This is what we call a _component column_, and it corresponds to the [Scalar](../../reference/types/components/scalar.md) component logged to the `/blendshapes/0/jawOpen` entity.
- Each row in the `/blendshapes/0/jawOpen:Scalar` column consists of a _list_ of (typically one) scalar.

This last point may come as a surprise but is a consequence of Rerun's data model where components are always stored as arrays. This enables, for example, to log an entire point cloud using the [`Points3D`](../../reference/types/archetypes/points3d.md) archetype under a single entity and at a single timestamp.

Let's explore this further, recalling that, in our recording, no face was detected at around frame #170:

```python
print(df["/blendshapes/0/jawOpen:Scalar"][160:180])
```

Here is the result:

```
160      [0.0397215373814106]
161    [0.037685077637434006]
162      [0.0402931347489357]
163     [0.04329492896795273]
164      [0.0394592322409153]
165    [0.020853394642472267]
166                        []
167                        []
168                        []
169                        []
170                        []
171                        []
172                        []
173                        []
174                        []
175                        []
176                        []
177                        []
178                        []
179                        []
Name: /blendshapes/0/jawOpen:Scalar, dtype: object
```

We note that the data contains empty lists when no face is detected. When the blendshapes entities are [`Clear`](../../reference/types/archetypes/clear.md)ed, this happens for the corresponding timestamps and all further timestamps until a new value is logged.

While this data representation is in general useful, a flat floating point representation with NaN for missing values is typically more convenient for scalar data. This is achieved using the [`explode()`](https://pandas.pydata.org/pandas-docs/stable/reference/api/pandas.DataFrame.explode.html) method:

```python
df["jawOpen"] = df["/blendshapes/0/jawOpen:Scalar"].explode().astype(float)
print(df["jawOpen"][160:180])
```
Here is the result:
```
160    0.039722
161    0.037685
162    0.040293
163    0.043295
164    0.039459
165    0.020853
166         NaN
167         NaN
168         NaN
169         NaN
170         NaN
171         NaN
172         NaN
173         NaN
174         NaN
175         NaN
176         NaN
177         NaN
178         NaN
179         NaN
Name: jawOpen, dtype: float64
```

This confirms that the newly created `"jawOpen"` column now contains regular, 64-bit float numbers, and missing values are represented by NaNs.

_Note_: should you want to filter out the NaNs, you may use the [`dropna()`](https://pandas.pydata.org/pandas-docs/stable/reference/api/pandas.DataFrame.dropna.html) method.

## Next steps

With this, we are ready to analyze the data and log back the result to the Rerun viewer, which is covered in the [next section](analyze-and-log.md) of this guide.
