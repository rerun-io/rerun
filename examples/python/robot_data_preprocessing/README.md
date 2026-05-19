<!--[metadata]
title = "Robot data preprocessing example"
tags = ["API example"]
thumbnail = "https://static.rerun.io/robot_postprocessing_thumb/ae27d24c3f530e71ed15ce47745eda56312ad014/480w.png"
thumbnail_dimensions = [480, 299]
-->

This example demonstrates how Rerun's [chunk processing API](https://rerun.io/docs/concepts/logging-and-ingestion/chunk-processing-api) can be used to assemble a robot recording from multiple file sources, including preprocessing to modify or augment the data.

<picture>
  <img src="https://static.rerun.io/robot_postprocessing_thumb/ae27d24c3f530e71ed15ce47745eda56312ad014/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/robot_postprocessing_thumb/ae27d24c3f530e71ed15ce47745eda56312ad014/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/robot_postprocessing_thumb/ae27d24c3f530e71ed15ce47745eda56312ad014/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/robot_postprocessing_thumb/ae27d24c3f530e71ed15ce47745eda56312ad014/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/robot_postprocessing_thumb/ae27d24c3f530e71ed15ce47745eda56312ad014/1200w.png">
</picture>

## Introduction

### Input data

While the example uses simulated data, it's intentionally designed to cover real-world challenges that should sound familiar to most roboticists:

- incomplete data requiring preprocessing
- custom data types
- bugs in the recorded data
- data spread across multiple files in different formats

Specifically, we use a recording of a dual-robot-arm setup, consisting of:

| `episode.mcap` | `offsets.json` | URDF files |
| --- | --- | --- |
| Base recording (videos, sensors, …). | Static world offsets for each robot. | Robot & scene models as [URDF](https://en.wikipedia.org/wiki/URDF).
| Some cameras have wrong parameters.<br>No dynamic 3D transforms were recorded,<br>only joint states in a custom Protobuf schema. |  Saved outside of base recording. | `robot.urdf`, `scene.urdf`, mesh data |

### Goals

Our task is to handle and process all the different data sources:

- read, convert and fix MCAP data
- compute 3D transforms using MCAP joint states and URDF
- handle URDFs
    - add `scene.urdf` and 2x `robot.urdf`
    - modify visual meshes with a custom color & transparency per robot
- add static transforms from JSON

…and merge them into one coherent recording.

## Processing pipeline

Solving such a task in an elegant way requires a non-trivial amount of engineering, but Rerun's [chunk processing API](https://rerun.io/docs/concepts/logging-and-ingestion/chunk-processing-api) gives us all the tools to properly structure the pipeline:

<!-- Figma: https://www.figma.com/board/xOvrUjklsfPH8OB3GUDnax/Michael-s-scratchpad-%F0%9F%91%A8%F0%9F%8F%BB%E2%80%8D%F0%9F%8E%A8?node-id=0-1&t=3uvGqaikPNKr80iH-1 -->

<picture>
  <img src="https://static.rerun.io/robot_postprocessing/722231a7e1523c45f22a2fa4162a9e960df88f08/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/robot_postprocessing/722231a7e1523c45f22a2fa4162a9e960df88f08/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/robot_postprocessing/722231a7e1523c45f22a2fa4162a9e960df88f08/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/robot_postprocessing/722231a7e1523c45f22a2fa4162a9e960df88f08/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/robot_postprocessing/722231a7e1523c45f22a2fa4162a9e960df88f08/1200w.png">
</picture>

The example code implements this pipeline and contains several explanatory comments.
We recommend reading the concept explanations below, before going through the `main()` function of [`robot_data_preprocessing.py`](robot_data_preprocessing.py) to understand the code structure.

> ℹ️ Note that we create two separate RRD files in this example.
For the Rerun Viewer or Catalog, both *physical* files form one [*logical* recording](https://rerun.io/docs/concepts/logging-and-ingestion/recordings#logical-vs-physical-recordings) since they specify the same recording ID.

### Chunk streams

[Chunks](https://rerun.io/docs/concepts/logging-and-ingestion/chunks) are the core datastructure of Rerun.
In this example, we use [chunk _streams_](https://rerun.io/docs/concepts/logging-and-ingestion/chunk-processing-api) as the "glue" of our pipeline.

In a nutshell, `LazyChunkStream`s allow us to define how `Chunk`s get routed through filtering, transformation and output steps.
As the name suggests, these streams are lazily evaluated.
We use an expressive Python API to define the pipeline, but the final execution happens in a multithreaded, GIL-free execution engine written in Rust for maximum efficiency.

In this example, we use the following sources that can emit `LazyChunkStream`s:
* `McapReader.stream()` for the MCAP recording
* `UrdfTree.stream()` for the URDF models
* manually constructed `LazyChunkStream` for the custom JSON file, using [`Chunk.from_columns(…)`](https://rerun.io/docs/concepts/logging-and-ingestion/chunks#sending-actual-chunks-sendchunks)

### Lenses

[Lenses](https://rerun.io/docs/concepts/query-and-transform/lenses) allow us to modify the chunks' components via [`MutateLens`](https://rerun.io/docs/concepts/query-and-transform/lenses#mutate-lenses), or to derive completely new components from them via [`DeriveLens`](https://rerun.io/docs/concepts/query-and-transform/lenses#derive-lenses).
In both cases, we use [`Selector`](https://rerun.io/docs/concepts/query-and-transform/lenses#selectors)s to extract component fields we're interested in, and pipe them through custom transformation functions.

#### `MutateLens` example

A simple `MutateLens` used in this example is the one that fixes the swapped `Pinhole:resolution` component of the external camera streams:
```python
mcap_stream.lenses(
    MutateLens(
        "Pinhole:resolution",
        Selector(".").pipe(
            lambda resolution: pa.array(
                [(height, width) for width, height in resolution.to_pylist()], type=resolution.type
            )
        ),
    ),
    content=["/external/cam_low", "/external/cam_high"],
    output_mode="forward_unmatched",
)
```
The `content` filter makes sure that this lens only gets applied to the external camera entities, while the `output_mode` makes sure we forward the other pinhole entities that don't match unchanged (here: the robot cameras that don't require the fix).

#### `DeriveLens` example

A more complex lens setup is required for the forward kinematics, i.e. to compute the 3D transforms from joint values (angles, distances).
For this we need the recorded joint states from the MCAP, as well as the URDF for the kinematic structure.

Our MCAP file contains joint states encoded in a custom Protobuf schema:
```proto
message JointState {
  google.protobuf.Timestamp timestamp = 1;
  repeated string joint_names = 2;
  repeated double joint_positions = 3;
  repeated double joint_velocities = 4;
  repeated double joint_efforts = 5;
}
```
This custom schema is not part of the [directly supported message types](https://rerun.io/docs/concepts/logging-and-ingestion/mcap/message-formats) of the MCAP importer (like e.g. the video streams). But thanks to [schema reflection](https://rerun.io/docs/concepts/logging-and-ingestion/mcap/message-formats#schema-reflection), we still get chunks with queryable Rerun components that we can process in our streams.

Each input row of joint states contains `N` joint values that map to `N` 3D transforms, for which we want to have a dedicated output row with [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d) each.
Due to this input-to-output row length mismatch, we use two sequential lenses:
1. For each joint state message…
    * select the joint names and values
    * use [`UrdfTree.compute_joint_transform_batches`](https://ref.rerun.io/docs/python/stable/urdf/#rerun.urdf.UrdfTree.compute_joint_transform_batches)
    * output a single row with a list of `N` 3D transforms.
2. Scatter each computed row into `N` rows with `Transform3D` component columns.

#### Others

Besides fixing camera data and computing forward kinematics, we also apply lenses for smaller things like URDF model colorization.

Finally, the streams are merged and written to two RRD files with the same recording ID to form layers of a single [logical recording](https://rerun.io/docs/concepts/logging-and-ingestion/recordings#logical-vs-physical-recordings).
We use two RRDs for demonstration purposes, but merging into a single RRD would be also possible.

See the code for all implementation details.

## Run the code

```bash
pip install -e examples/python/robot_data_preprocessing
python -m robot_data_preprocessing
```

The resulting RRDs can be opened in the viewer:
```bash
rerun examples/python/robot_data_preprocessing/output/*.rrd
```
Since we use consistent recording IDs, the two output RRD layers show up as a single recording.

<image src="https://static.rerun.io/e8b3975732ed5f42e125b0c80b487e97d8e99041_robot_postprocessing.gif" width=500/>
<!-- MP4 version: https://static.rerun.io/5e0d6be2f9a1c21686ff8177a9085c6858ec3f74_robot_postprocessing.mp4 -->

## Summary

We showed how a non-trivial robotics problem can be solved through a structured data pipeline.
The chunk processing API provides the tools to build such custom pipelines in a compact manner (here: < 200 lines of Python code) while having a powerful execution engine under the hood.

We also demonstrated how recording IDs can be used to structure RRDs into logical recordings, allowing also to potentially add more layers (e.g. for metadata or extra sensor data).

Documentation links for further reading:
* [Chunk processing API](https://rerun.io/docs/concepts/logging-and-ingestion/chunk-processing-api)
* [Lenses API](https://rerun.io/docs/concepts/query-and-transform/lenses)
* [Recordings](https://rerun.io/docs/concepts/logging-and-ingestion/recordings)
* [Working with MCAP](https://rerun.io/docs/howto/logging-and-ingestion/mcap)
* [Loading URDF models](https://rerun.io/docs/howto/logging-and-ingestion/urdf)

## Going further

In a real-world setting, this kind of processing would be only the first step of data curation, to finalize multiple raw recordings before ingesting them to central storage.

With Rerun, this would mean registering a dataset to a [catalog server](https://rerun.io/docs/concepts/how-does-rerun-work#catalog-server) (either via Rerun Hub for enterprise scalability, or using the open-source `rerun server` for small-scale local development).
This enables e.g. to perform [queries across recordings](https://rerun.io/docs/concepts/query-and-transform/dataframe-queries) for analytics or to export training data.
