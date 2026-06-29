---
name: rerun-catalog-queries
description: Performance patterns and gotchas for querying a Rerun catalog from Python. Reach for this when a CatalogClient/dataset query is unexpectedly slow, or when shaping a per-segment / per-episode pipeline that hits the catalog from many places.
user_invocable: true
allowed-tools: Read, Grep, Bash
---

# Rerun catalog queries

Practical performance patterns for querying a Rerun catalog
from Python (`rerun.catalog.CatalogClient` →
`dataset.reader(...)` → DataFusion `DataFrame`). The DataFusion side of
the stack is covered by the **`datafusion-python`** skill — load that
for `DataFrame` / `SessionContext` / expression-API references. This
skill focuses on catalog-specific behaviors and the round-trip costs
that catch teams off guard.

---

## The query cost model in one sentence

Every materialization of a `dataset.reader(...)` DataFrame is **one
cloud round-trip**, with most of the cost being the network/decode
pair, not the compute. Plan for round-trip count and payload bytes, in
that order.

A typical catalog round-trip is **a few seconds** even for a tiny
result. So:
- 30 segments × 1 query each ≈ 90s. (Naive per-segment loop.)
- 30 segments × 4 queries each ≈ 6 minutes. (A splitter that runs
  `count` + `collect_column` for both starts and stops.)
- 1 query covering all 30 segments ≈ 3s.

The same fan-out happens along the **entity** axis: 10 entities ×
1 `filter_contents([one_entity]).reader()` each ≈ 30s, vs one
`filter_contents([all_entities]).reader()` ≈ 3s. Push as much as you
can into one round-trip — across segments and across entities.

---

## Always apply `filter_contents` and time-window filters before `.reader(...)`

The single biggest lever:

- `filter_contents([entity_globs])` restricts which entity-path columns
  the reader produces. Without it, every entity in the dataset is
  read.
- For Scalars-typed columns this also reduces array nesting depth from
  `list<list<double>>` to `list<double>`.
- Time-window filters (`df.filter(col(index).cast(int64) >= start)
  .filter(col(index).cast(int64) <= end)`) push down to storage and
  dramatically reduce bytes scanned. The order matters: filter then
  reader-bound projection, never the other way around.

Combined: `dataset.filter_segments(seg).filter_contents(entities).reader(...)
.filter(in_window).select(...)`.

---

## `df.cache()` is your friend for repeated probes

When the same materialization gets used by multiple downstream
filter/count/collect calls, materialize once with `DataFrame.cache()`
and operate on the cached frame:

```python
cached = (
    dataset
    .filter_segments(seg)
    .filter_contents([entity])
    .reader(index=index_col)
    .select(col(index_col).cast(pa.int64()).alias(index_col), value.alias("v"))
    .cache()  # one network round-trip, materializes into in-memory batches
)
starts = cached.filter(col("v") == start_val).collect_column(index_col)
stops = cached.filter(stop_pred(col("v"))).collect_column(index_col)
```

Without `cache()`, each `count()` / `collect_column()` re-executes the
whole reader chain.

**When NOT to cache.** `cache()` forces materialization into Arrow
batches, breaking laziness. If downstream code keeps composing more
DataFusion ops on top (joins, windows, further filters) and only
materializes once at the end, caching mid-pipeline turns one execution
into two and pre-empts whatever physical-plan optimizations the engine
could have done across the boundary. Reach for `cache()` when the
consumers are terminal (`count()`, `collect_column()`, `to_arrow_table()`),
not when they're another lazy `DataFrame`.

---

## Cross-segment batching: drop `filter_segments`, group by `rerun_segment_id`

For pipelines that need the same query on many segments, omit
`filter_segments(...)` entirely and pull a single cross-segment table.
Every reader row carries a `rerun_segment_id` column — group locally:

```python
df = dataset.filter_contents(entities).reader(index=index_col)
cached = df.select(
    "rerun_segment_id",
    col(index_col).cast(pa.int64()).alias(index_col),
    value.alias("v"),
).cache()

# Now N filter/aggregate calls are local, not network.
starts = cached.filter(col("v") == start_val).select("rerun_segment_id", index_col).to_arrow_table()
```

Trigger / event columns are tiny enough that pulling all segments at
once dominates per-segment looping by an order of magnitude.

---

## Per-entity fan-out within a segment

Symmetric to cross-segment batching, along the entity axis. If you
need data from N entities of a single segment, **don't** loop:

```python
# Anti-pattern: N reader setups, N round-trips.
for entity in entities:
    df = dataset.filter_segments(seg).filter_contents([entity]).reader(index=ix)
    ...
```

Instead pull all N at once and project per entity locally. Per "Reader
row layout" below, every row carries data for one entity and NULLs
for the others, so `col("<entity>:<archetype>:<component>").is_not_null()`
is the per-entity filter:

```python
shared = (
    dataset
    .filter_segments(seg)
    .filter_contents(sorted(set(entities)))
    .reader(index=ix)
    .filter(col(ix).cast(pa.int64()).between(start_ns, end_ns))
)
# Each downstream consumer narrows to its entity's rows lazily.
src_a = shared.filter(col(f"{ent_a}:{comp_a}").is_not_null()).select(ix, f"{ent_a}:{comp_a}")
src_b = shared.filter(col(f"{ent_b}:{comp_b}").is_not_null()).select(ix, f"{ent_b}:{comp_b}")
```

DataFusion can share the underlying scan across the per-entity
projections when it builds the physical plan, so this stays a single
catalog round-trip even though there are N logical consumers. Works
inside generators that build per-source DataFusion plans (resampling,
bracket lookup, nearest-in-time joins) — collapse the network fan-out
without changing the per-source logic.

A trap when refactoring: if a downstream query uses a reader column's
fully-qualified name (`col(f"{entity}:{archetype}:{component}")`),
you don't need to alias the column in `shared`. The shared reader's
output schema preserves native column names, so existing per-entity
projection helpers keep working unchanged.

---

## `count()` is *not* free

Counter-intuitive: `df.count()` and `df.aggregate([], [F.count(col)])`
do not always push down. Aggregate plans can force the engine to
materialize the underlying column data server-side, then count on the
client. `F.count(col)` over wide entity-columns can ship full struct or
blob payloads to count nullity.

Alternatives, in order of preference for "is anything here":

| Need | Use |
|---|---|
| "any row in this filter?" | `df.select(col(index)).limit(1).to_arrow_table().num_rows > 0` — server short-circuits on first match |
| "count rows in a tiny window" | `df.filter(window).select(col(index)).count()` after the time filter |
| "count *each* entity in a wide query" | per-entity `limit(1)` probes, threaded — *not* one big `count(col)` aggregate |

A trap that fooled us: assuming `bool_or(col.is_not_null())` would only
need nullity buffers. It does not — the operator still touches payload
data on most plans.

---

## `using_index_values` + `fill_latest_at` is great for resampling, not for presence

```python
.reader(index=index_col, using_index_values=targets, fill_latest_at=True)
```

- Returns one row per target timestamp, each entity column carrying its
  latest non-null value at-or-before the target.
- Excellent for nearest-prior resampling (no DataFusion required).
- **Don't** use it as a presence check. Two reasons:
  1. Semantics are "ever emitted before T", not "emitted in [start, T]".
  2. The server still ships the full struct/blob payload for every
     entity to compute the latest-known value; a downstream
     `is_not_null()` projection runs post-transfer and doesn't reduce
     wire bytes.

For a strict in-window presence check, prefer per-entity `limit(1)`
over the time-filtered reader, run concurrently.

---

## Schema introspection is cheap; use it before probing

```python
schema = dataset.filter_segments(seg).schema()
available = {(c.entity_path, c.component) for c in schema.component_columns()}
```

This is a single round-trip and tells you which `(entity, component)`
pairs the segment registered. If a column isn't in the schema, you can
drop it from your manifest without any further cloud queries. This is
often a complete substitute for "does this entity have events"
probing.

Caveat: schema presence ≠ events. The MCAP records the topic schema
even for unused topics. If your pipeline cares about distinguishing
"registered but never emitted" vs "registered with events", you have
to probe — see "is anything here" patterns above.

---

## Reader row layout: entities are columns, not rows

Every row from `dataset.<filters>.reader(...)` corresponds to a single
event on a single entity. Other entities' columns are null on that
row. Implications:

- `select("rerun_segment_id", "<entity>:<archetype>:<component>")`
  works — quote the entity column when using SQL.
- There is **no** `rerun_entity_path` row attribute. To attribute rows
  to entities you either filter to one entity at a time, or pick a
  per-entity column (e.g. `:McapChannel:id`) whose non-null pattern
  identifies the source.
- `df.count()` returns *total events across all entities*, not
  per-entity counts.

---

## Field access on a null struct returns `0.0`, not null

A DataFusion gotcha that bites pipelines reading struct messages:

```python
col("/some/entity:msg.MyType:message")[0]["sub"]["x"]
# When the parent struct is null on a row, this evaluates to 0.0
# (and "" for strings), not null.
```

Wrap struct-walk projections with a null guard:

```python
parent = col("/some/entity:msg.MyType:message")
leaf = parent[0]["sub"]["x"]
guard = parent.is_null() | parent[0].is_null() | parent[0]["sub"].is_null()
expr = F.when(guard, lit(None)).otherwise(leaf)
```

**Only apply this to struct sources.** For scalar / blob columns
(`Scalars:scalars`, `EncodedImage:blob`, etc.) the wrap is a no-op at
best and at worst rewrites the plan in ways that change downstream
join behavior. Gate the guard on whether the source actually walks
through a struct boundary.

---

## Common debug recipe

When a query stage is slower than expected:

1. **Count round-trips.** Wrap each `to_arrow_table()` /
   `collect_column()` / `count()` with `time.perf_counter()`. Each is
   a round-trip. If you see N segment queries, that's N × few-seconds
   minimum.
2. **Split build vs materialize timing.** Time the lazy DataFrame
   construction *separately* from the terminal `to_arrow_table()` call.
   If "build" takes seconds, something inside is materializing eagerly
   (an Arrow round-trip in a join helper, a `cache()` in a generator,
   a `.collect()` hidden in a chained-join utility). A correctly lazy
   plan should build in ~milliseconds regardless of result size.
3. **Measure bytes.** `tbl.nbytes` after `to_arrow_table()` reveals
   when "I projected `is_not_null()`" actually shipped megabytes. If
   bytes are large despite a small projection, the operator didn't
   push down.
4. **Cross-segment first, then cross-entity.** If the per-segment query
   is fundamentally the same (just scoped by id), drop `filter_segments`
   and group by `rerun_segment_id` locally. If you also have a
   per-entity loop within a segment, collapse it the same way (one
   `filter_contents([all])` reader, per-entity `is_not_null()` filters
   downstream).
5. **Cache before terminal re-use.** If two `count()` / `collect_column()`
   calls share the same reader, `df.cache()` between them. Don't cache
   if the consumers are themselves lazy DataFrames being composed
   further — caching breaks plan-wide optimization.
6. **Window first.** Always push the time filter before any projection
   or aggregate that touches payload columns.

---

## See also

- `datafusion-python` skill — DataFrame API, SQL parity, expression
  building, common pitfalls (boolean operators, immutability, etc.).
  Not installed? Ask the user to install it globally:
  `npx skills add apache/datafusion-python`.
