---
title: Optimize memory footprint & ingestion speed
order: 50
---


<!-- TODO: link to 0.18 release also -->

## Data model recap

Rerun stores data into [`Chunk`s](https://docs.rs/rerun/latest/rerun/log/struct.Chunk.html).

A `Chunk` is a collection of columns, similar to e.g. an [Arrow `RecordBatch`](https://docs.rs/arrow/latest/arrow/array/struct.RecordBatch.html), a [Polars `DataFrame`](https://docs.rs/polars/latest/polars/frame/struct.DataFrame.html?search=series), or any other columnar datastructure of that nature that you might already be familiar with.

["Logging data"](../getting-started/data-in) into Rerun really means creating `Chunk`s and indexing them into the Rerun datastore.

In order to be indexable, `Chunk`s always carry around with them a fixed set of metadata (schema, temporal indices, column statistics, etc).
The corollary to this is that there is a fixed cost, both in terms of compute and memory footprint, to creating `Chunk`s, regardless of the amount of actual data you put in them.

Similarly, because the Rerun datastore works a the `Chunk`-level, there are fixed space and compute costs to be paid every time a `Chunk` is ingested and indexed.
THe corollary is that the overhead induced by these fixed costs will be directly proportional to how much actual data is present within these `Chunk`s.

Put simply: the larger the `Chunk`s, the lower the memory footprint and the faster the ingestion speeds.

Which raises the question: how does one go about creating larger `Chunk`s?

## Creating and ingesting `Chunk`s

We will be logging a highly detailed sine wave made up of 100k samples as a motivating example for this guide:
<!-- TODO: take a final non-shitty screenshot -->
<html>
  <picture>
    <img src="https://static.rerun.io/ingest_guide_sine/ad297cb42c8e082b0f49df24857a719af0c812eb/full.png" alt="">
    <source media="(max-width: 480px)" srcset="https://static.rerun.io/ingest_guide_sine/ad297cb42c8e082b0f49df24857a719af0c812eb/480w.png">
    <source media="(max-width: 768px)" srcset="https://static.rerun.io/ingest_guide_sine/ad297cb42c8e082b0f49df24857a719af0c812eb/768w.png">
    <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ingest_guide_sine/ad297cb42c8e082b0f49df24857a719af0c812eb/1024w.png">
    <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ingest_guide_sine/ad297cb42c8e082b0f49df24857a719af0c812eb/1200w.png">
  </picture>
</html>

We will start with the most naive, and unsurprisingly slowest, approach and work our way up in performance by introducing new techniques, until most of the overhead is out of our way.


### Using the Log APIs

<!-- * Logging data the silly way -->
<!--   * Enter the SDK batcher (link to micro-batching docs) -->
<!--   * Enter the Chunk Store compactor -->

The most basic approach is to iteratively generate 100k values and log them as they come:
<!-- TODO: actual snippet for all languages -->
snippet: howto/ingestion-guide/log_rows.rs

You can run this code and ingest the data into the viewer with the following:
<!-- TODO: actual snippet for all languages -->
snippet: howto/ingestion-guide/run_example_spawn.rs

This is of course extremely inefficient, both from a general programming standpoint (we're allocating temporary objects and paying for a bunch of function calls for every single scalar we log) and from a Rerun data model standpoint (clearly it doesn't look like we're creating the largest possible `Chunk`s here!).

Still, depending on the machine you run this on and the programming language you use, you might find the performance to be perfectly fine.
In fact, running the Rust example on my machine is pretty much instantaneous (both logging and ingestion), and the memory overhead (~30MB for the entire viewer) seems totally acceptable. How is that possible?


#### SDK micro-batching & Chunk Store compaction

The Log APIs are built from the ground up for real-time use cases, i.e. when your data is being generated at the same time as you're logging it.
To make these use cases as efficient as possible, two major optimizations happen in the background:
* [SDK micro-batching](../reference/sdk-micro-batching)
* Chunk store compaction

The SDK micro-batcher is a background thread that runs directly inside the SDK and which accumulates log calls for a set amount of time.
By default, the batcher accumulates log calls for 8 milliseconds before yielding a new `Chunk`.

Using the log APIs is not all bad though: in fact, in many use cases, it is the most appropriate choice, e.g. when your data is being generated in real-time as you're logging it.

The reason this works at all is because the Rerun SDK embeds a [micro-batcher](../../content/concepts)

<!-- TODO: cargo r --release | rerun rrd print -->


<!-- TODO: demonstrate effect of RERUN_FLUSH_NUM_ROWS -->
<!-- TODO: link to micro-batching guide -->

<!-- TODO: we also need to aside into the store compactor -->
<!-- TODO: I guess we also need to talk about the different settings? -->


### Using the send APIs

<!-- TODO: the fact that the dataset is known AOT is important to mention. -->

<!-- * Logging data the smart way -->
<!--   * Enter the `send_` APIs -->

<!-- TODO: actual snippet for all languages -->
snippet: howto/ingestion-guide/send_rows.rs

<!-- TODO: actual snippet for all languages -->
snippet: howto/ingestion-guide/run_example_spawn.rs


### Using the Rerun CLI

<!-- * Using the Rerun CLI to pre-compact the data -->
<!--   * Things are now optimal all around... -->
<!--   * ...but what if we could do this all at once? -->

What if you don't know your dataset AOT? What can you do then?


<!-- TODO: actual snippet for all languages -->
snippet: howto/ingestion-guide/run_example_save.rs

```
python log_rows.py | rerun rrd compact 
```

<!-- TODO: link to CLI ref -->
