---
title: Chunks
order: 700
---

<!-- TODO(cmc): talk about `send_dataframe` once it gets fleshed out a bit more -->

A *Chunk* is the core datastructure at the heart of Rerun: it dictates how data gets logged, injected, stored, and queried.
A basic understanding of chunks is important in order to understand why and how Rerun and its APIs work the way they work.


## How Rerun stores data

All the data you send into Rerun is stored in chunks, always.

A chunk is an [Arrow](https://arrow.apache.org/)-encoded, column-oriented table of binary data:

<picture>
  <img src="https://static.rerun.io/a_chunk/c3536f34028a9cc4976fa428d98c802fe3ac07a4/full.png" alt="A Rerun Chunk">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/a_chunk/c3536f34028a9cc4976fa428d98c802fe3ac07a4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/a_chunk/c3536f34028a9cc4976fa428d98c802fe3ac07a4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/a_chunk/c3536f34028a9cc4976fa428d98c802fe3ac07a4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/a_chunk/c3536f34028a9cc4976fa428d98c802fe3ac07a4/1200w.png">
</picture>

A *Component Column* contains one or more [*Component Batches*](batches.md), which in turn contain one or more instances (that is, a component is *always* an array). Each component batch corresponds to a single *Row ID* and one [time point per timeline](timelines.md).

This design allows for keeping chunks within a target size range, even for recordings that combine low frequency but large data like point clouds or tensors (wide columns), with high frequency but small signals (tall columns).

<picture>
  <img src="https://static.rerun.io/weird_chunks/ce98d89bfefbe59a816ae4650e634573d59cf34a/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/weird_chunks/ce98d89bfefbe59a816ae4650e634573d59cf34a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/weird_chunks/ce98d89bfefbe59a816ae4650e634573d59cf34a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/weird_chunks/ce98d89bfefbe59a816ae4650e634573d59cf34a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/weird_chunks/ce98d89bfefbe59a816ae4650e634573d59cf34a/1200w.png">
</picture>


Here's an excerpt from a real-world chunk (taken from the [Helix example](https://app.rerun.io/?url=https%3A%2F%2Fapp.rerun.io%2Fversion%2Flatest%2Fexamples%2Fdna.rrd)) (you might want to open [this image](https://static.rerun.io/a_real_chunk/2c4c16303dd1a04ba8ad8962ed85386a6568773e/full.png) in a new tab):

<picture>
  <img src="https://static.rerun.io/a_real_chunk/2c4c16303dd1a04ba8ad8962ed85386a6568773e/full.png" alt="A real-world Rerun chunk">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/a_real_chunk/2c4c16303dd1a04ba8ad8962ed85386a6568773e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/a_real_chunk/2c4c16303dd1a04ba8ad8962ed85386a6568773e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/a_real_chunk/2c4c16303dd1a04ba8ad8962ed85386a6568773e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/a_real_chunk/2c4c16303dd1a04ba8ad8962ed85386a6568773e/1200w.png">
</picture>

You can see that this matches very closely the diagram above:
* A single *control* column, that contains the globally unique row IDs.
* Multiple *time*/*index* columns (`log_tick`, `log_time`, `stable_time`).
* Multiple component columns (`Points3D:colors`, `Points3D:positions`, `Points3D:radii`).

Within each row of each component column, the individual cells are [*Component Batches*](batches.md). Component batches are the atomic unit of data in Rerun.

The data in this specific chunk was logged with the following code:

snippet: concepts/how_helix_was_logged

You can learn more about chunks and how they came to be in [this blog post](http://rerun.io/blog/column-chunks#storage-is-based-around-chunks-of-component-columns).


## Getting chunks into Rerun

If you've used the Rerun SDK before, you know it doesn't actually force to manually craft these chunks byte by byte - that would be rather cumbersome!

How does one create and store chunks in Rerun, then?


### The row-oriented way: `log`

The `log` API is generally [what we show in the getting-started guides](https://rerun.io/docs/getting-started/data-in/python#logging-your-own-data) since it's the easiest to use:

snippet: archetypes/scalars_row_updates

The `log` API makes it possible to send data into Rerun on a row-by-row basis, without requiring any extra effort.
This row-oriented interface makes it very easy to integrate into existing codebase and just start logging data as it comes (hence the name).

Reference:
* [🐍 Python `log`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log)
* [🦀 Rust `log`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log)
* [🌊 C++ `log`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a7badac918d44d66e04e948f38818ff11)

But if you're handing a bunch of rows of data over to Rerun, how does it end up neatly packaged in columnar chunks?


#### How are these rows turned into columns?

Before logging data, you can use the `rr.set_time_` APIs to update the SDK's time context with timestamps for custom timelines.
For example, `rr.set_time("frame", sequence=42)` will set the "frame" timeline's current value to 42 in the time context.

When you later call `rr.log`, the SDK will generate a row id and values for the built-in timelines `log_time` and `log_tick`.
It will also grab the current values for any custom timelines from the time context.
Any data passed to `rr.log` or `rr.log_components` becomes component batches.

<picture>
  <img src="https://static.rerun.io/build-row/c617d2b5c233c36ae78f723528c9e0cc3acf1bb0/full.png" alt="A diagram showing how a row gets created in Rerun">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/build-row/c617d2b5c233c36ae78f723528c9e0cc3acf1bb0/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/build-row/c617d2b5c233c36ae78f723528c9e0cc3acf1bb0/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/build-row/c617d2b5c233c36ae78f723528c9e0cc3acf1bb0/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/build-row/c617d2b5c233c36ae78f723528c9e0cc3acf1bb0/1200w.png">
</picture>

The row id, timestamps, and logged component batches are then encoded as Apache Arrow arrays and together make up a row.
That row is then passed to a batcher, which appends the values from the row to the current chunk for the entity path.

<picture>
  <img src="https://static.rerun.io/build-chunk/b5a7e1c15a814add0a42c9d77e82f2a44aba585c/full.png" alt="A diagram showing how a chunk gets created in Rerun">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/build-chunk/b5a7e1c15a814add0a42c9d77e82f2a44aba585c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/build-chunk/b5a7e1c15a814add0a42c9d77e82f2a44aba585c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/build-chunk/b5a7e1c15a814add0a42c9d77e82f2a44aba585c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/build-chunk/b5a7e1c15a814add0a42c9d77e82f2a44aba585c/1200w.png">
</picture>

The current chunk is then sent to its destination, either periodically or as soon as it crosses a size threshold.
Building up small column chunks before sending from the SDK trades off a small amount of latency and memory use in favor of more efficient transfer and ingestion.
You can read about how to configure the batcher [here](../../reference/sdk/micro-batching.md).

### The column-oriented way: `send_columns`

The `log` API showcased above is designed to extract data from your running code as it's being generated. It is, by nature, *row-oriented*.
If you already have data stored in something more *column-oriented*, it can be both a lot easier and more efficient to send it to Rerun in that form directly.

This is what the `send_columns` API is for: it lets you efficiently update the state of an entity over time, sending data for multiple index and component columns in a single operation.

> ⚠️ `send_columns` API bypasses the time context and [micro-batcher](../../reference/sdk/micro-batching.md) ⚠️
>
> In contrast to the `log` API, `send_columns` does NOT add any other timelines to the data. Neither the built-in timelines `log_time` and `log_tick`, nor any [user timelines](timelines.md). Only the timelines explicitly included in the call to `send_columns` will be included.

snippet: archetypes/scalars_column_updates

See also the reference:
* [🐍 Python `send_columns`](https://ref.rerun.io/docs/python/0.21.0/common/columnar_api/#rerun.send_columns)
* [🦀 Rust `send_columns`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.send_columns)
* [🌊 C++ `send_columns`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a7e326526d1473c02fcb2ed94afe6da69)
