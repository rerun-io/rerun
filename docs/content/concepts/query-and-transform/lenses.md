---
title: Lenses
order: 400
---

> **Note:** The Lenses API is currently experimental and may change in future releases.

Lenses transform data by extracting, reshaping, and rerouting components.
They produce new component columns, entity paths, or timelines from existing data.

## Motivation

The goal of Rerun is to handle all kinds and shapes of user data.
In addition to the datatypes defined by Rerun, it is also possible to load data with user-defined types into the viewer and into `.rrd` files.
For example, [schema reflection](../logging-and-ingestion/mcap/message-formats.md#schema-reflection) can be used to import arbitrary Protobuf-based MCAP messages.
Using an expressive API, Lenses allow you to:

1. Reroute components to different entities
2. Attach Rerun semantics to arbitrary data
3. Wrangle the values stored in individual components

Lenses are available in the Rust SDK using `LensesSink` or directly on a `Chunk` via the `ChunkExt` trait.
In Python, Lenses can be applied to chunks directly or as a pipeline step in the `ChunkStream` API.

Internally, Rerun uses lenses to implement large parts of our data importers, the MCAP importer is one example of this.

## Example data

The examples below all operate on the same input chunk, logged to `/sensor/imu` with `frame` as a timeline and two component columns `Imu:accel` and `Imu:status`:

snippet: concepts/lenses[log_data]

| `frame` | `Imu:accel` | `Imu:status` |
|------:|-----------|------------|
| 0 | `[{x: 1.0, y: 4.0, elapsed: 0}]` | `["ok"]` |
| 1 | `[{x: 2.0, y: 5.0, elapsed: 10000000}]` | `["ok"]` |
| 2 | `[{x: 3.0, y: 6.0, elapsed: 20000000}]` | `["warn"]` |

## Derive lenses

A derive lens creates **new** component columns from an input component.
It selects an input column, extracts data using a `Selector`, and writes the results as new columns (optionally at a different entity and with additional timelines).

The following lens extracts the `.y` field from the struct as a [`Scalar`](../../reference/types/archetypes/scalars.md), extracts the `.elapsed` field as a new timeline, and writes both to the entity `/new_entity/accel_y`:

snippet: concepts/lenses[derive_lens]

See the full examples in [Rust](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/concepts/lenses.rs?speculative-link) and [Python](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/concepts/lenses.py?speculative-link).

When we apply the `extract_y` lens, we get the following resulting chunks.

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

## Mutate lenses

A mutate lens modifies an existing component column by applying a selector to it.
Unlike derive lenses, no new columns are created.
The input column is transformed and stays at the same entity.

The following lens simplifies the `Imu:accel` struct to just its `.x` field:

snippet: concepts/lenses[mutate_lens]

After applying the `simplify_accel` lens, `/sensor/imu` looks like this:

| `frame` | `Imu:accel` | `Imu:status` |
|------:|-----------|------------|
| 0 | `[1.0]` | `["ok"]` |
| 1 | `[2.0]` | `["ok"]` |
| 2 | `[3.0]` | `["warn"]` |

The struct has been replaced by the extracted float values, while `Imu:status` remains unchanged.

## Output modes

When streaming data through lenses, the output mode controls which components are forwarded:

* `ForwardUnmatched` forwards original components that are not consumed by any lens, alongside any lens-produced outputs.
* `ForwardAll` forwards all original components alongside lens-produced outputs. This leads to data duplication but can be helpful for debugging.
* `DropUnmatched` only forwards lens-produced outputs, dropping all other components.


## Selectors

The actual transformations of the contents and values within a given column are expressed using `Selectors`, which are concise, declarative expressions that are inspired by [`jq`](https://jqlang.org/).
Because a lot of user-defined types are hierarchically nested message definitions, this yields a natural way to describe extractions.

The basic syntax elements are:

* `.` - identity, selects the current value
* `.field` - access a named field (e.g. `.my.nested.struct.field`)
* `.sequence[]` - iterate over all elements in a sequence
* `.sequence[].x` - access a field on each element of a sequence
* `.optional_field?` - access an optional field, skipping missing values

These can be composed using pipes (`|`) as described below.

### Pipe

The `|` operator pipes the output of one expression into the next, just like a Unix pipe.
In the query string, this is useful for readability when chaining multiple steps: `.poses[] | .x`.

Beyond the query syntax, `Selector.pipe()` can also chain into arbitrary functions in the host language.
This is useful for value transformations that go beyond path navigation, like unit conversions or arithmetic.
For example, the following lens extracts the `.x` field and scales it by `9.81`:

snippet: concepts/lenses[pipe_example]
