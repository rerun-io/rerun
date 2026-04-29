---
title: Lenses
order: 400
---

> **Note:** The Lenses API is currently experimental and may change in future releases.

Lenses transform chunk data by extracting, reshaping, and rerouting components.
They operate on individual chunks and produce one or more output chunks with new component columns, entity paths, or timelines.

## Motivation

The goal of Rerun is to handle all kinds and shapes of user data.
In addition to the datatypes defined by Rerun, it is also possible to load data with user-defined types into the viewer and into `.rrd` files.
For example, [schema reflection](../logging-and-ingestion/mcap/message-formats.md#schema-reflection) can be used to import arbitrary Protobuf-based MCAP messages.
Using an expressive API, Lenses allow you to:

1. Reroute components to different entities
2. Attach Rerun semantics to arbitrary data
3. Wrangle the values stored in individual components

Lenses are available in the Rust SDK using `LensesSink` or directly on a `Chunk` via the `ChunkExt` trait.
<!-- TODO(RR-4502): Link to ChunkStream docs once available. -->
In Python, Lenses can be applied to chunks directly or as a pipeline step in the `ChunkStream` API.

Internally, Rerun uses lenses to implement large parts of our data importers, the MCAP importer is one example of this.

## Operational model

Lenses operate on (component) columns and generally consist of these steps:

1. Select an input column using a `ComponentIdentifier`.
2. Choose a target `ComponentDescriptor` to describe the semantics of the resulting column.
3. The transform operations that are performed on the input as a `Selector`.

## Example

Here is an example of what this looks like in our SDKs.
Let's assume we have data that was logged like the following:

snippet: concepts/lenses[log_data]

This produces a chunk on `/sensor/imu`, with `frame` as a timeline and two component columns `Imu:accel` and `Imu:status`:

| `frame` | `Imu:accel` | `Imu:status` |
|------:|-----------|------------|
| 0 | `[{x: 1.0, y: 4.0, elapsed: 0}]` | `["ok"]` |
| 1 | `[{x: 2.0, y: 5.0, elapsed: 10000000}]` | `["ok"]` |
| 2 | `[{x: 3.0, y: 6.0, elapsed: 20000000}]` | `["warn"]` |

We can now define a lens for this data, which extracts the `.y` field from the struct as a component, extracts the `.elapsed` field as a timeline, tags it as a Rerun [`Scalar`](../../reference/types/archetypes/scalars.md), and moves the result to the new entity `/new_entity/accel_y`.
In code, the lens will look like this:

snippet: concepts/lenses[lens_definition]

See the full examples in [Rust](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/concepts/lenses.rs?speculative-link) and [Python](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/concepts/lenses.py?speculative-link).

<!-- TODO(RR-4442): Revise this section once we have settled on the correct chunk handling. -->
<!-- TODO(RR-4481): Mention `inplace` lens output. -->

When we apply the `extract_y` lens, we get the following resulting components.

On `/sensor/imu`, the unmodified `Imu:status` column remains:

| `frame` | `Imu:status` |
|------:|------------|
| 0 | `["ok"]` |
| 1 | `["ok"]` |
| 2 | `["warn"]` |

On `/new_entity/accel_y`, we get the extracted [`Scalar`](../../reference/types/archetypes/scalars.md) column and the new `sensor_elapsed` timeline:

| `frame` | `sensor_elapsed` | `Scalars:scalars` |
|------:|------:|-----------|
| 0 | 0 | `[4.0]` |
| 1 | 10000000 | `[5.0]` |
| 2 | 20000000 | `[6.0]` |

Note that the original `frame` timeline is present for all entities with the correct values.

### Output modes

In chunk streaming scenarios, users can specify what should happen with matched and unmatched chunks.
Lenses support the following output modes:

* `ForwardUnmatched` will forward unmatched chunks as well as a residual chunk that contains all components that are not targeted by any Lens.
* `ForwardAll` will forward all chunks unconditionally. This will lead to data duplication but can be helpful, for example for debugging.
* `DropUnmatched` all chunks and components that are not targeted by any lens will be discarded.


## Selectors

The actual transformations of the contents and values within a given column are expressed using `Selectors`, which are concise, declarative expressions that are inspired by [`jq`](https://jqlang.org/).
Because a lot of user-defined types are hierarchically nested message definitions, this yields a natural way to describe extractions.

The basic syntax elements are:

* `.` — identity, selects the current value
* `.field` — access a named field (e.g. `.my.nested.struct.field`)
* `.sequence[]` — iterate over all elements in a sequence
* `.sequence[].x` — access a field on each element of a sequence
* `.optional_field?` — access an optional field, skipping missing values

These can be composed using pipes (`|`) as described below.

### Pipe

The `|` operator pipes the output of one expression into the next, just like a Unix pipe.
In the query string, this is useful for readability when chaining multiple steps: `.poses[] | .x`.

Beyond the query syntax, `Selector.pipe()` can also chain into arbitrary functions in the host language.
This is useful for value transformations that go beyond path navigation, like unit conversions or arithmetic.
For example, the following lens extracts the `.x` field and scales it by `9.81`:

snippet: concepts/lenses[pipe_example]
