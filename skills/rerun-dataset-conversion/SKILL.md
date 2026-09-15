---
name: rerun-dataset-conversion
description: "Convert a multi-modal robotics dataset (MCAP, HDF5, LeRobot, parquet, raw video) into layered Rerun recordings (.rrd) and a catalog-ready dataset. Use whenever the user wants a dataset converted or ingested into Rerun, a conversion pipeline reviewed or extended, or a layer added to an existing conversion — even when they only say 'convert X to rrd' or 'ingest this dataset'. Routes to the rerun-* skills for Rerun mechanics."
---

# Dataset conversion

This skill guides the conversion of a multi-modal robotics dataset into layered Rerun recordings (`.rrd`).
It covers the high-level workflow guidelines: surveying the source, building layers, designing the default blueprint, registering a catalog, and validating the result.
It also provides DOs and DON'Ts for each stage of the conversion process.

## Terms

The skill and its references use these terms throughout:

| Term           | Meaning                                                                                                   |
| -------------- | --------------------------------------------------------------------------------------------------------- |
| episode        | The source's natural unit of recording.                                                                   |
| recording id   | The string that identifies one episode's recordings. All of the episode's layers share it.                |
| layer          | One `.rrd` file per episode. The viewer and the catalog stack an episode's layers into one recording.     |
| base layer     | The layer that reflects the source. It is the one record, sufficient without the source.                  |
| segment        | One episode as the catalog sees it: its layers stacked under one recording id.                            |
| property       | A per-episode value stored as a recording property. The catalog shows it as a column.                     |
| census         | A count of decoded rows against the source's own counts, so a silent drop surfaces.                       |
| survey         | The phase that measures sample diversity before any conversion code exists.                               |

## What this skill adds, and what it routes away

This skill adds how a conversion is shaped, decided, and validated, and which judgment calls recur from one dataset to the next.

The `rerun-*` skills cover the Rerun mechanics, and this skill routes to them instead of repeating them.
Read each one at the stage that needs it:

| Skill                                                                         | When to read it                                                                                                         |
| ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `rerun-data-model`                                                            | Before modeling any source item. It settles entity versus component, static versus temporal, and property versus layer. |
| `rerun-chunk-processing`                                                      | Before writing any conversion code. It mandates reader + lens pipelines and carries the anti-pattern list.              |
| `rerun-mcap` / `rerun-parquet` / `rerun-lerobot` / `rerun-urdf` / `rerun-mp4` | When the source has that format.                                                                                        |
| `rerun-blueprint`                                                             | When designing the default view.                                                                                        |
| `rerun-catalog-queries`                                                       | When registering or querying a catalog.                                                                                 |

A conversion project's conventions belong in that project's own docs. The skill only carries principles and decision points.

## Expected output at the end of data conversion

- Code that downloads or loads, converts (produces the 'base', a reflection of the source), and enriches (produces other 'layers' and a blueprint).
- Documentation that helps users run the code, understand the source data, and use the converted data.

## Principles

Every dataset is unique, which makes it hard to set universal guidelines.
The principles below aim to provide a foundation that you can revisit when encountering decision challenges during dataset conversion.

### 1. Conversion must be lossless.

The conversion process should preserve all information from the source dataset.
Dropping data is a failure, and the byte-level information must survive a round-trip test and be sufficient to reconstruct the source data.
After the conversion, no looking back at the original source.

### 2. Make it easy to use and maintain.

The primary purpose of conversion is to use the data, not to archive it.
The data should be organized so that it is easy to inspect, query, or access for further consumption.
How the data is processed and organized may also evolve over time. So making the conversion code maintainable matters, too.

### 3. Efficient.

Without the correct tools, data may drop silently, and it may become hard to maintain. (Failure of the first two items)
Also, efficiency matters at large scale.
Stick to the rerun skills listed above.

## Workflow

### Step 0: gather information

Obtain any information available even before touching any data.
For example:

- Storage layout from the data source (e.g. the dataset card)
- Data file formats and modalities
- Context of the data (e.g. tasks, hardware platform, environment)
- Extra resources: the accompanying paper, the project website, any notes on the dataset by the authors

> **DO NOT** assume these information to be correct until you have thoroughly examined actual samples.

### Step 1: obtain sample data

1. Ask the user what can be the maximum sample size to download. Some datasets are very large, and downloading the entire dataset may be impractical.
2. Write a download script (with caching for faster lookup and options to select what to download).
3. Download random samples across different tasks and sessions.

### Step 2: understand your data — expect the unexpected

1. Investigate the samples.
   Understand their underlying format and contents.
2. Survey the diversity (ranges or distributions) of the samples.
   Common dimensions to look at:
   - FPS
   - Image size and format. If video, resolution and encoding spec (GOP, codec)
   - Missing / extra topics
   - Empty or invalid values
   - Anything off from what's expected or very unusual
   - Metadata consistency
3. Record the survey in a readable format (markdown) for the user's review as well as future reference.
   Keep a list of exemplar episodes for testing.
   Pause for user to review the survey.

> **DO NOT** assume that one sample is representative, and avoid overfitting to a few samples.

### Step 3: conversion - base

1. Design the conversion mapping based on relevant skills:`rerun-mcap` / `rerun-parquet` / `rerun-lerobot` / `rerun-urdf` / `rerun-mp4`.

> **DO NOT** force unsupported data into an unrelated archetype.
> Preserve the original meaning and structure as much as possible.
> Rerun can store arbitrary Arrow-compatible data even when the Viewer has no first-class understanding of it.

See [Data Mapping Guidelines](#guidelines-data-mapping).

2. Plan to record the source metadata as part of the base layer and write them as properties for easy query on the catalog.
   Plan to add useful static per-episode facts as properties.
   Optional sensor data may be written as a separate layer (e.g. IR cameras that appear in only 20% of the sessions).

Pause for user to sign-off on the conversion mapping, properties record, and layer splits before writing any conversion code.

3. Write the conversion code using the skills. Include properties and layer splits as agreed upon in the previous step.

> **DO NOT** call `to_chunks()` in conversion code. The call materialises the whole stream before the first iteration.
> Do not explode a struct into per-field lenses, and give every `collect()` a stated reason.

### Step 4: conversion check

1. Add round-trip tests to ensure the conversion is lossless. (original -> base -> original)
2. Compare the file sizes: source vs base.
   Investigate if there is a gap between source and base.

### Step 5: initial blueprint

1. Write an initial draft blueprint: put views for the major sensing modalities (images, positions, texts) and other information (e.g. annotations) useful for visual inspection.
2. Check the viewer performance and the correctness of labels.
3. Iterate with the user as needed.

> **DO** remind the user to investigate if the viewer performance drops.

### Step 6 [optional]: enrich the data

1. Decide whether to include more visual elements (URDF) or post-processed data as extra layers based on user's needs.
2. Update the blueprint to include new layers as needed.

> **DO** check the viewer performance and the correctness of labels with the user for every major blueprint update.

### Step 7: use the data

Write scripts or provide instructions to help the user to view, register, and query the converted data.

`references/registering.md` carries the view commands, the registration template, and where querying is covered.

## Guidelines: data mapping

### Default to reflection

Reflect the reader's output, and convert to a built-in Rerun archetype only when the semantics perfectly match or particular visualization is critical.
Do not distort the stored schema just to make it easy to visualize, and preserve useful source semantics that may be useful later.
Prefer duplicate data in a different format over dropping it if needed.
Image or video data can be an exception because duplicated copies can be costly.
Let the user decide when the conversion is not straightforward.

### Prefer queryable structure

As long as your data can be serialized with Apache Arrow, Rerun can log it.
Use raw blobs mainly when preserving the original bytes is important or parsing is impractical.
For example, a JSON file with a simple schema can be kept as a structured component rather than a raw blob.
For another example, a set of strings joined by commas can be kept as a list component rather than a single string.
This allows individual elements to be queried and visualized.

### Make only lossless format changes

In general, reshaping is fine as far as the byte-level information is preserved — a stamp becoming a timeline value, an image frame flipped upright.
Keep names, units, frames, and source type names, because they are the context that makes values interpretable.
Ensure round-trip conversion tests are done.

### For undecodable channels

Run census undecodable channels and keep their bytes.
A decoder that skips a message must not skip silently: compare decoded rows against the source's own counts, flag the episode in a property, and store the raw bytes.

### Properties

Properties are first and foremost segment-level metadata, which can be quickly searched in a catalog.
Write static values that a user would filter, sort, or group as properties so they can be efficiently queried and displayed in the catalog.
Properties can be, for example, episode duration, task name, scene type, split, `is_<validity>`, `has_<attribute>`and the
census.
A value that changes within the episode cannot be a property.

Build them with `Chunk.from_property`; `rerun-chunk-processing` has the call and
the trap in the hand-built form.

Keep a property's type identical across every episode.
A field that is empty in one episode and populated in another must still land the
same Arrow type, or the layer stops having one schema across segments and the
catalog column splits.

## Guidelines: enrichment

### Splitting into layers

See [Why Use Layers](https://rerun.io/docs/howto/logging-and-ingestion/layers#why-use-layers) to get a sense first.

- One base layer for one episode conversion is enough.
- If not part of the raw conversion, write as separate layers.
  In particular, temporally separate recordings should be separate layers. (e.g., When annotations are added later, write it as a separate layer from the base layer.)
- Even if the output can be produced at the same time when the base layer is produced, if it is heavily dependent on external resources rather than part of the conversion processing (e.g. URDF layer), write it as a separate layer.

There is no universal rule for when to create a separate layer; make suggestions based on the guidelines above and let the user make the final decision.
