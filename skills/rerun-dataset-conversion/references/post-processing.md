# Post-processing: deriving a layer from existing RRD data

Ask about the user's data and operation in plain words, choose between chunk processing and a dataframe query from the answers, and explain the choice by referring to those answers.

## 1. Ask the user

Ask only what the context doesn't already answer.

1. Where is the dataset: RRD files on this machine, or a catalog such as Rerun Hub?
   The answer decides how to open the data and where to save the new layer.
   It does not decide whether to use chunk processing or a dataframe query.
2. Can each output value be computed from one sample alone?
    - Yes: converting units, renaming a field, resizing each image, computing an embedding for each image, when each image is its own sample (JPEG, PNG, or raw pixels).
    - No: smoothing, computing velocity from positions, matching camera frames to joint states, anything on the frames of a video, or anything else that needs earlier samples or other sensors.
      To decode a video frame, the decoder needs every sample back to the last keyframe, and the codec is stored apart from the samples.

    The answer decides whether to use chunk processing or a dataframe query.
    If the user can't tell, treat the answer as no.

3. Will the job run on one machine or on many workers?
   The answer decides how to register the result.

## 2. Choose chunk processing or a dataframe query

| Dataset   | Each value from one sample                             | Needs other samples or sensors                                           |
| --------- | ------------------------------------------------------ | ------------------------------------------------------------------------ |
| RRD files | Lenses on `RrdReader(path).stream()`                   | `RrdReader(path).stream().collect()`, then `ChunkStore.reader(index=…)`  |
| Catalog   | Lenses on `dataset.segment_store(segment_id).stream()` | `dataset.filter_segments(segment_id).filter_contents(…).reader(index=…)` |

Chunk processing sees one chunk at a time, whether it runs lenses or `stream.map(…)`.
Which rows share a chunk depends on how the file was written, not on the data, so chunk processing is correct only when each output value depends on its own input sample.
A dataframe query returns the same rows however the data is chunked, so it works for both answers.

On a catalog, query one segment at a time: post-processing writes one file per segment, often reads large columns such as video, and can give each worker one segment.
For small columns on one machine, one query across all segments is faster, as `rerun-catalog-queries` recommends; split its result by `rerun_segment_id` (see step 3).

On a stream, select the entities the operation reads with `.filter(content=…)` right after `.stream()`.
For a catalog segment, only the matching chunks are then downloaded.
For RRD files, the dataframe query needs `collect()`, because `ChunkStore.reader()` only queries data held in memory.
Open a catalog segment with `segment_store(segment_id, include_assets=False)`, so the stream leaves out the dataset's shared assets, such as a robot model.

Do not download the catalog's RRD files to process them.
`rerun-chunk-processing` covers streams and lenses.
`rerun-catalog-queries` covers catalog queries.

Some operations can't be done with Arrow functions, such as decoding video, composing images, or running a model.
Convert the samples to the form the operation needs, and convert the result back to Arrow before writing.
With chunk processing, do this inside the lens; with a dataframe query, do it on the query's result.
Work on the Arrow buffers directly instead of converting values to Python objects with `.to_pylist()`.

For a video, query the `VideoStream:sample` column in time order and decode from a keyframe onward, one GOP at a time.
A GOP (group of pictures) is a keyframe and the frames that depend on it.
The `VideoStream:is_keyframe` column marks where each GOP starts, so a long video can be decoded GOP by GOP without holding every frame in memory.
Write each result at the timestamp of the frame it came from.
[Query video streams](https://rerun.io/docs/howto/query-and-transform/query_videos) shows the query and the decode loop.

## 3. Write the layer

Write one RRD file per segment, with the source segment's ID as `recording_id`.
On a catalog, register the file under a new `layer_name`.
Write derived values to a new entity or component, and keep the lens default `output_mode="drop_unmatched"`, so the layer holds only the derived data and no copy of the source.

A chunk-processing pipeline already produces chunks.
A dataframe result is the exception to `rerun-chunk-processing`'s rule that readers and lenses produce all chunks: it needs other samples, and a lens sees one chunk at a time.
Turn it back into chunks with `Chunk.from_dataframe`:

- A query across segments has a `rerun_segment_id` column.
  Split the rows by it, and drop the column before writing.
  Otherwise `Chunk.from_dataframe` turns it into a component on the root entity `/`.
- If the timeline column has no Rerun index metadata, as in a table built from scratch, pass it as `index=`, e.g. `Chunk.from_dataframe(table, index="frame")`.
- Keep the names of columns that came from a query, since they already contain their entity path and component.
- Name a new column `<entity path>:<component>`, e.g. `/embedding/top-camera:embedding`.
- For a built-in archetype the viewer must draw, such as `EncodedImage`, build the columns with the archetype's `columns(…)` helper and pass them to `Chunk.from_columns`.
  A plain column has no archetype, so the viewer would not draw it.

Then save the file:

- Catalog: Write the file with `OptimizationProfile.OBJECT_STORE` (see `rerun-chunk-processing`), then stage it in the catalog's storage.
  With many workers, each worker writes and stages its own segment's file.
  `registering.md` covers how to stage and register, and when a worker should register its own file.
- RRD files: Save the file next to the base RRDs.
  Register it later if the dataset moves to a catalog.
