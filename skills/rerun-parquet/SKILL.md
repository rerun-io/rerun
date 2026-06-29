---
name: rerun-parquet
description: Ingest tabular Parquet files into Rerun chunk streams with rerun.experimental.ParquetReader. Read when converting trajectory or sensor tables (LeRobot-style parquet, exported logs) into entities and components — column grouping, timeline/index columns, static columns, and lenses (DeriveLens) that assemble the typed components (Transform3D, Scalars) from the reader's grouped struct/scalar output. Builds on rerun-chunk-processing and rerun-data-model.
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun parquet ingestion

`ParquetReader` is a **pure reader**: it maps a flat table onto the Rerun
model by turning raw columns into grouped, time-indexed chunks of struct and
scalar components. Column-name prefixes become entities, grouped columns
become a single struct component, designated columns become timelines. The
reader does **not** assemble archetypes anymore — mapping struct fields into
typed Rerun components (Transform3D, Scalars, Points3D) is done with lenses on
the reader's `.stream()`. The whole reader job is configuration; fill in the
`rerun-data-model` mapping table first, then express it through the
constructor. Stream mechanics after `.stream()` are in
`rerun-chunk-processing`.

**The whole table is configuration, not code.** If you find yourself building
`Chunk.from_columns` from a parquet, or munging it in pandas first, stop —
`ParquetReader` plus a lens almost certainly expresses it. Anything the reader
cannot express (per-row entity routing, derived values, unit conversion)
belongs in lenses downstream, not in pre-pandas munging; keep the pipeline
columnar.

## The API

```python
from rerun.experimental import ParquetReader, DeriveLens

reader = ParquetReader(
    table_path,
    entity_path_prefix="/world",  # prepended to every entity path
    column_grouping="prefix",  # "prefix" | "individual" | "explicit_prefixes"
    delimiter="_",  # split for column_grouping="prefix"
    prefixes=None,  # required for "explicit_prefixes"
    use_structs=True,  # pack grouped columns into one struct component
    static_columns=["robot_type"],  # constant-per-file values, logged static
    index_columns=[("timestamp", "timestamp", "us"), ("frame_index", "sequence")],
)
stream = reader.stream()
```

Every parameter after `path` is keyword-only. There is no `column_rules`
kwarg — typed-component assembly moved to lenses (below).

## What the reader emits

The reader turns the table into chunks, one chunk per group, then leaves the
data as generic struct/scalar components for lenses to map. The naming is the
key thing the rest of the pipeline keys off:

- **A grouped multi-column prefix `X`** → entity `/X`, with a single struct
  component named **`data`**. The struct's fields are the column names with the
  prefix (and delimiter, for `"prefix"`) stripped. So `A_pos_x`, `A_quat_w`
  under prefix `A` land as struct `data` with fields `pos_x`, `quat_w` on
  entity `/A`.
- **A lone column with no group** → its own entity named after the column, and
  a raw component named after the column — *not* a `data` struct. So a `speed`
  column becomes entity `/speed`, component `speed`.
- **A `/__properties` metadata chunk** built from the parquet file's schema
  metadata. You typically drop it right after `.stream()`:

  ```python
  stream = reader.stream().drop(content="/__properties/**")
  ```

## Column grouping: which columns share an entity

- `"prefix"` (default): split each column name on `delimiter`, group by the
  first segment. `gripper_pos_x`, `gripper_pos_y` → entity `/gripper`, struct
  `data{pos_x, pos_y}`.
- `"explicit_prefixes"`: group by the exact strings in `prefixes`, tried
  longest-first; the prefix is stripped from each struct field name (a raw
  string match, no delimiter — `foo` + `a` → field `a`). Columns matching no
  prefix become individual groups. Use this when names contain the delimiter
  ambiguously (`observation.state` vs `observation.images.top`: pass the full
  prefixes).
- `"individual"`: every column is its own chunk/entity with a raw component
  named after the column — no struct packing at all, even for columns sharing a
  prefix. `use_structs` is ignored here. Rarely the model you want; reach for
  it only as a debugging baseline.

`use_structs=True` (default) packs a group's columns into a single Arrow
struct component (the `data` field) for `"prefix"`/`"explicit_prefixes"`;
`False` emits one component per column (the pre-struct flat layout, what
queries see as separate columns).

## Timelines: `index_columns`

Each entry is `(name, type)` or `(name, type, unit)`:

- `type`: `"timestamp"` (since epoch), `"duration"` (elapsed), `"sequence"`
  (ordinal int).
- `unit` describes what the raw integers in the column *are* (`"ns"` default,
  `"us"`, `"ms"`, `"s"`); Rerun rescales to ns internally. Ignored for
  `"sequence"`.

**If omitted, a synthetic `row_index` sequence timeline is generated.** That
is almost never the timeline you want to query or align against; always name
the real time columns. Stamp both a timestamp and a sequence timeline when the
table has both (multi-rate alignment, see `rerun-data-model`).

## Static columns: `static_columns`

Listed columns are constant across all rows; they are emitted once as a single
static (timeless) chunk, separate from the temporal data. A listed column that
actually varies raises an error when the stream runs — that error is a
data-quality signal, not a reason to drop the static declaration.

## Typed components via lenses

The reader's grouped output is generic struct (`data`) and scalar data. A
`DeriveLens` reads that struct's fields, packs and casts them into real Rerun
components, and writes them to an output entity — this is what the old
`column_rules` API used to do, now done downstream on the stream.

Construct a lens against the reader's struct component (`"data"` for grouped
prefixes, or the column name for a lone/individual column), then add one or
more `.to_*` builder methods. Each builder returns a fresh lens, so they chain.

| Builder | Produces | Argument order |
|---|---|---|
| `to_translation(x, y, z)` | `Transform3D:translation` | x, y, z |
| `to_quaternion(x, y, z, w)` | `Transform3D:quaternion` | x, y, z, w (xyzw) |
| `to_scale(x, y, z)` | `Transform3D:scale` | x, y, z |
| `to_rotation_axis_angle(axis_x, axis_y, axis_z, angle)` | `Transform3D:rotation_axis_angle` | axis_x, axis_y, axis_z, angle (radians) |
| `to_scalars(*fields)` | `Scalars:scalars` | one or more field names |
| `to_packed_component(component, *fields)` | the given component | descriptor, then field names |
| `to_component(component, selector)` | the given component | descriptor, then a `Selector` |
| `to_timeline(name, type, selector)` | a timeline (not a component) | name, `"sequence"`/`"duration_ns"`/`"timestamp_ns"`, selector |

`to_packed_component` packs the named struct fields (in order, at least one
required) into the fixed-size list the component expects, and by default
**auto-casts `f64`→`f32`** to match component types. The `to_translation`,
`to_quaternion`, `to_scale` helpers are convenience wrappers over it, so they
all auto-cast. `to_rotation_axis_angle` builds a `Struct{axis, angle}` and
hard-casts axis and angle to `f32` internally. `to_scalars` with a single field
emits a plain scalar per row (not a 1-element list); with multiple fields it
emits one scalar series per field at the same entity.

Apply lenses with `.stream().lenses([lens], content="/A", output_mode="drop_unmatched")`:

- `content` is a pre-filter on the *source* entity path — it scopes which
  chunks the lens may touch. Out-of-scope chunks pass through unchanged. Set it
  to the reader's grouped entity (e.g. `"/A"`).
- `output_mode` decides the fate of in-scope-but-unmatched chunks:
  `"drop_unmatched"` (default, keep only lens output), `"forward_unmatched"`
  (output replaces matched, other originals survive), or `"forward_all"`
  (output plus all originals).
- The lens's own `output_entity=` sets the *destination* entity — independent
  of `content`, which gates the input side.

End-to-end Transform3D example. The reader groups `A_*` columns into a `data`
struct at `/A`; the lens reads the prefix-stripped field names (`pos_x`,
`quat_w`), packs and casts them, and writes a full `Transform3D` to `/pose`:

```python
from rerun.experimental import DeriveLens, ParquetReader

lens = (
    DeriveLens("data", output_entity="/pose")
    .to_translation("pos_x", "pos_y", "pos_z")
    .to_quaternion("quat_x", "quat_y", "quat_z", "quat_w")
)

chunks = (
    ParquetReader(table_path, index_columns=[("frame_index", "sequence")])
    .stream()
    .lenses([lens], content="/A", output_mode="drop_unmatched")
    .to_chunks()
)
```

Chaining several `.to_*` on **one lens with a shared `output_entity`**
accumulates multiple component columns into the same archetype at that entity —
above, both `Transform3D:translation` and `Transform3D:quaternion` land on
`/pose`, forming a complete `Transform3D`. For a generic fixed-size-list
component, pass the descriptor to `to_packed_component`:

```python
import rerun as rr
from rerun.experimental import DeriveLens, ParquetReader

lens = DeriveLens("data", output_entity="/points").to_packed_component(
    rr.Points3D.descriptor_positions(), "x", "y", "z"
)
```

## Selectors

Lens field paths use `Selector`, a jq-like grammar over Arrow columns
(`.field` to access a struct field, `[]` to iterate a list, `[N]` to index, `?`
to suppress errors on absent fields, `!` to assert non-null, `|` to pipe, and
`pack(.x, .y, .z)` to zip paths into a fixed-size list). The `to_*` helpers
build these selectors for you; reach for `to_component(component, Selector(".x"))`
when you need a custom field path. Field paths reference the
**prefix-stripped** struct field names — the lens sees `pos_x`, not `A_pos_x`.

## Gotchas

1. No `index_columns` → synthetic `row_index` timeline only. Queries that
   expect a timestamp timeline find nothing.
2. The `unit` is the raw column's unit, not a desired output unit; a
   microsecond column declared `"ns"` lands 1000x in the past.
3. `static_columns` raises if a listed column actually varies; that error is a
   data-quality signal, not a reason to drop the static declaration. It is
   raised lazily when the stream runs, not at construction.
4. A grouped prefix's struct component is named **`data`** — that is the
   `input_component` string a `DeriveLens` matches against. A lone or
   `"individual"` column is instead a raw component named after the column.
5. Selector field paths reference the **prefix-stripped** struct field names
   (`pos_x`, not `gripper_pos_x`).
6. Drop the `/__properties` metadata chunk the reader emits from parquet schema
   metadata: `.stream().drop(content="/__properties/**")`.
7. Quaternion column order is x, y, z, w in `to_quaternion`; check the source's
   convention before wiring fields.
8. `to_packed_component` (and the transform helpers built on it) auto-casts
   `f64`→`f32` to match component types; this is usually what you want for
   parquet's double columns.
9. Anything the reader cannot express (per-row entity routing, derived values,
   unit conversion) belongs in lenses downstream, not in pre-pandas munging;
   keep the pipeline columnar (`rerun-chunk-processing`).

## References

- Lens builder source with full docstrings: `rerun/experimental/_lens.py` in
  the installed `rerun-sdk` package (`to_translation`, `to_quaternion`,
  `to_scale`, `to_rotation_axis_angle`, `to_scalars`, `to_packed_component`,
  `to_component`, `to_timeline`).
- Reader source: `rerun/experimental/_parquet_reader.py`, or
  `python -c "from rerun.experimental import ParquetReader; help(ParquetReader)"`
- Canonical worked examples: the integration tests
  `rerun_py/tests/integration/test_parquet_reader.py` (grouping, index/static
  columns, and the Transform3D / Points3D / Scalars lens flows) and
  `rerun_py/tests/integration/test_lazy_chunk_stream.py` (lens application,
  `content`/`output_mode`, selectors).
- `rerun-lerobot` — LeRobot datasets store episodes as parquet; that skill
  covers the built-in importer route vs reading the parquet directly with
  this reader.
- `rerun-data-model` (mapping decisions), `rerun-chunk-processing` (stream
  mechanics after `.stream()`)
