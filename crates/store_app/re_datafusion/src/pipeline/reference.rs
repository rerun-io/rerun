//! Test oracle for how the dataframe query turns chunks into rows.
//!
//! A dataframe query emits rows on one index timeline, one cell per selected
//! `(entity, component)` column. When a column holds several values at one
//! time — a `Transform3D` logged for many child frames in separate log calls
//! at one time, say — the query has to decide which survive and whether that
//! time becomes several rows. The rules below make that decision.
//!
//! This module states those rules as plainly as possible, with a full scan of
//! every chunk and no indexes or caches, so that a reviewer can check each
//! rule against the code. The columnar row emitter ([`super::columnar`]) is tested
//! by comparing its output with [`evaluate`] on the same chunks.
//!
//! With no driving column, this module agrees with `QueryHandle`, and the
//! tests compare the two wherever no driving column is set. There are three
//! exceptions, all under rule 3: `QueryHandle` does not apply a static
//! `Clear` to a value at its own logged time
//! (<https://github.com/rerun-io/rerun/issues/7650>), it shows the value
//! of a row that logs a value and a `Clear` together, and it applies a
//! recursive `Clear` on an ancestor, a deprecated feature.
//!
//! Only queries on a timeline are modelled; a query with no index returns one
//! row of static data per segment and is out of scope.
//!
//! The output has one row per time at which a selected column logged a
//! temporal value or was cleared — only the `filtered_is_not_null` column's
//! times if the query names one — within the query's index range. A column
//! with a static value adds no times of its own, only those of its clears
//! (rules 1 and 3).
//!
//! The **driving column**, if the query names one, is the one column allowed
//! to hold several values at a time: each of its values becomes its own row.
//! Its values also say which rows describe the same thing: two rows with
//! equal driving values at one time are one thing logged twice. It should
//! therefore be a column whose values name things, such as
//! `Transform3D:child_frame`; on a column of measurements, equal measurements
//! would merge. A value is one logged cell with all its instances: a row that
//! logs three child frames in one batch is one value and becomes one row.
//!
//! The rules below refer to this example, on timeline `frame`. Chunks are
//! logged in the order A, B, C, D, so later chunks have higher `RowId`s.
//!
//! | chunk | `frame` | `/tf:child_frame` | `/tf:translation` |
//! |-------|---------|-------------------|-------------------|
//! | A     | 20      | `a`               | 1                 |
//! | A     | 20      | `b`               | 2                 |
//! | C     | 20      | `c`               | 3                 |
//! | D     | 40      | `a`               | 11                |
//!
//! | chunk | `frame` | `/joint:value` |
//! |-------|---------|----------------|
//! | B     | 10      | 1              |
//! | B     | 20      | 2              |
//! | B     | 30      | 3              |
//! | B     | 40      | 4              |
//!
//! Selecting `/tf:child_frame` as the driving column, plus `/tf:translation`
//! and `/joint:value`, without fill:
//!
//! | `frame` | `/tf:child_frame` | `/tf:translation` | `/joint:value` |
//! |---------|-------------------|-------------------|----------------|
//! | 10      | null              | null              | 1              |
//! | 20      | `a`               | 1                 | 2              |
//! | 20      | `b`               | 2                 | 2              |
//! | 20      | `c`               | 3                 | 2              |
//! | 30      | null              | null              | 3              |
//! | 40      | `a`               | 11                | 4              |
//!
//! With `fill_latest_at`, `frame = 30` becomes three rows (`a`/1, `b`/2,
//! `c`/3) and `frame = 40` becomes `b`/2, `c`/3, `a`/11, in `RowId` order.
//!
//! Rules, in evaluation order:
//!
//! 1. A static value wins over every temporal one for its column, and among
//!    static values the highest `RowId` wins. This is the documented meaning
//!    of static data: <https://rerun.io/docs/concepts/logging-and-ingestion/static>.
//!    On a timeline, a static value never creates a row, and the temporal
//!    rows it shadows create none either, even once a static `Clear` has
//!    emptied it: a column with a static value only fills rows that other
//!    columns, or clears on its entity, create.
//! 2. Rows of a column at the same time correct one another, and the highest
//!    `RowId` wins. The driving column decides which rows count as the same
//!    thing:
//!    * In the driving column, only rows with equal values correct each
//!      other, whichever chunks they are in; rows with different values are
//!      all kept. In the example, `a` and `b` from chunk A and `c` from chunk C
//!      are three values at `frame = 20`; a later row logging `a` at
//!      `frame = 20` would replace chunk A's.
//!    * A column logged in at least one row with the driving column, such as
//!      `/tf:translation` next to `/tf:child_frame`, is not reduced at all:
//!      it follows the driving column row by row (rule 5). A row is what one
//!      log call writes; which chunk it ends up in does not matter.
//!    * Every other column, including one on the same entity that is logged
//!      in separate rows, treats all its rows at one time as corrections.
//!
//!    Without a driving column, every column is in the last group, so the
//!    rule cannot tell a correction from several things logged at one time.
//!    In the example, `frame = 20` then shows only `c`/3 in both `/tf`
//!    columns, and `a` and `b` are dropped without an error. Naming a driving
//!    column is how to keep them.
//! 3. A temporal `Clear` on the entity counts as an empty value and competes
//!    on `(time, RowId)` like any other row; a static `Clear` makes the
//!    column null instead (below).
//!    * A `Clear` on an ancestor never applies, whatever its recursive flag
//!      says.
//!    * A row that logs a value together with a `Clear` on its entity
//!      contradicts itself; the `Clear` wins.
//!    * A static `Clear` applies at every time: the column is null wherever
//!      it would show a temporal value or a static value logged before the
//!      `Clear`, and only a static value logged after it shows. This is how a
//!      latest-at query reads a static `Clear`.
//!    * Unlike a static value (rule 1), a static `Clear` leaves the grid
//!      alone: the cleared column's temporal times still create rows, which
//!      show it as null.
//!    * Rule 1 holds for the `Clear` itself: a static `Clear` on an entity
//!      shadows every temporal `Clear` on that entity, which then neither
//!      creates rows nor clears anything.
//!    * A `Clear` never clears its own column, `Clear:is_recursive`, which a
//!      query selects only when it asks for tombstone columns. That column
//!      reads like any other: a temporal `Clear`'s flag shows at its time and
//!      fills forward, and a static `Clear`'s flag is a static value.
//! 4. With `fill_latest_at`, the driving column shows the latest row per
//!    value at or before each time, and any other column with nothing at a
//!    time carries its latest value forward. In the example, chunk D updates
//!    `a` at `frame = 40` while `b` and `c` keep their rows from `frame = 20`.
//!    A clear shows as empty at its own time only; later times with nothing
//!    newer are null, matching `QueryHandle`.
//! 5. Each driving value becomes one row. A column logged in at least one row
//!    with the driving column shows, on each driving value's row, the value
//!    logged in the same row as the driving value, or null if that row did
//!    not log it. Every other column repeats its single value. In the
//!    example, `/tf:translation` shows 1, 2 and 3 next to `a`, `b` and `c`,
//!    and `/joint:value` repeats `2` on each. A time at which the driving
//!    column has nothing, or only a `Clear`, is one row with the driving cell
//!    null or empty and every other column showing its own value.
//!    `filtered_is_not_null` picks the times, not the rows: if it names a
//!    following column, a driving value whose row did not log that column
//!    still gets a row, with that column null.
//! 6. Every timeline other than the index shows, per row, the latest time on
//!    it among the rows the row's cells show, a `Clear` included. A null
//!    cell, a static value and a row that a newer row or a `Clear` hides add
//!    nothing, so a value hidden by a static `Clear` does not move it; a
//!    value carried forward by fill adds the time of the row it came from.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef};
use arrow::row::{RowConverter, SortField};
use re_dataframe::external::re_chunk::{Chunk, RowId};
use re_log_types::{AbsoluteTimeRange, TimeInt, TimelineName};
use re_sorbet::ComponentColumnSelector;
use re_types_core::{ComponentIdentifier, archetypes};

/// The component `column` selects.
///
/// Panics on an invalid component string; selectors built with
/// [`ComponentColumnSelector::from_descriptor`] are always valid.
fn component_of(column: &ComponentColumnSelector) -> ComponentIdentifier {
    column
        .component_identifier()
        .unwrap_or_else(|err| panic!("invalid component in {column:?}: {err}"))
}

/// One dataframe query, in the subset of `QueryExpression` the reference models.
pub struct ReferenceQuery {
    /// The timeline the rows are on.
    pub index: TimelineName,

    /// The selected columns, in output order.
    pub columns: Vec<ComponentColumnSelector>,

    /// `filtered_index_range`: only times inside it (inclusive) become rows;
    /// `None` keeps every time. Fill still reads values logged before it.
    pub range: Option<AbsoluteTimeRange>,

    /// `filtered_is_not_null`, as an index into `columns`: the grid is that
    /// column's times instead of the union.
    pub filtered_is_not_null: Option<usize>,

    /// `SparseFillStrategy::LatestAtGlobal`: carry values forward to times
    /// at which a column logged nothing (rule 4).
    pub fill_latest_at: bool,

    /// Index into `columns` of the driving column, if any.
    pub driving: Option<usize>,
}

/// Where one output cell comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// A `Clear` won: an empty list of the column's type.
    Empty,

    /// The column has nothing to show on this row.
    Null,

    /// Row `row` of `chunks[chunk]`.
    Value { chunk: usize, row: usize },
}

/// One output row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceRow {
    /// The row's time on the index timeline.
    pub time: TimeInt,

    /// One cell per [`ReferenceQuery::columns`] entry, in that order.
    pub cells: Vec<Cell>,

    /// Per other timeline, the latest time of the rows whose value or
    /// `Clear` the row shows (rule 6); a timeline none of them has is absent.
    pub other_times: BTreeMap<TimelineName, TimeInt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FactKind {
    Clear,
    Data,
}

/// One source row that can supply a cell: data or a clear.
#[derive(Clone, Debug)]
struct Fact {
    /// `None` for static rows.
    time: Option<TimeInt>,
    row_id: RowId,
    chunk: usize,
    row: usize,

    /// The logged value in Arrow's row format, which is equal exactly when
    /// the values are; set only for the driving column.
    driving_value: Option<Vec<u8>>,
    kind: FactKind,
}

impl Fact {
    fn stamp(&self) -> (TimeInt, RowId) {
        (self.time.unwrap_or(TimeInt::STATIC), self.row_id)
    }

    fn cell(&self) -> Cell {
        match self.kind {
            FactKind::Data => Cell::Value {
                chunk: self.chunk,
                row: self.row,
            },
            FactKind::Clear => Cell::Empty,
        }
    }
}

fn facts_for(
    chunks: &[Arc<Chunk>],
    index: TimelineName,
    column: &ComponentColumnSelector,
    driving: bool,
) -> Vec<Fact> {
    let component = component_of(column);
    let clear_component = archetypes::Clear::descriptor_is_recursive().component;
    let own_chunks = || {
        chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.entity_path() == &column.entity_path)
    };
    // Rule 1 among static `Clear`s: only the latest one counts.
    let latest_static_clear = own_chunks()
        .filter(|(_, chunk)| chunk.is_static())
        .filter_map(|(_, chunk)| {
            let list = chunk.components().get_array(clear_component)?;
            (0..chunk.num_rows())
                .filter(|&row| !list.is_null(row))
                .map(|row| chunk.row_ids_slice()[row])
                .max()
        })
        .max();
    let mut facts = Vec::new();

    for (chunk_idx, chunk) in own_chunks() {
        let times = chunk.timelines().get(&index);
        if !chunk.is_static() && times.is_none() {
            continue;
        }
        let time_at = |row: usize| times.map(|t| TimeInt::new_temporal(t.times_raw()[row]));

        if let Some(list) = chunk.components().get_array(component) {
            let encoded = driving.then(|| {
                RowConverter::new(vec![SortField::new(list.data_type().clone())])
                    .and_then(|converter| converter.convert_columns(&[Arc::new(list.clone())]))
                    .expect("the driving column's datatype has a row encoding")
            });
            for row in 0..chunk.num_rows() {
                if list.is_null(row) {
                    continue;
                }
                facts.push(Fact {
                    time: time_at(row),
                    row_id: chunk.row_ids_slice()[row],
                    chunk: chunk_idx,
                    row,
                    driving_value: encoded.as_ref().map(|rows| rows.row(row).as_ref().to_vec()),
                    kind: FactKind::Data,
                });
            }
        }

        let own_column = component == clear_component;
        let shadowed = !chunk.is_static() && latest_static_clear.is_some();
        if !own_column
            && !shadowed
            && let Some(list) = chunk.components().get_array(clear_component)
        {
            for row in 0..chunk.num_rows() {
                if list.is_null(row)
                    || (chunk.is_static()
                        && latest_static_clear != Some(chunk.row_ids_slice()[row]))
                {
                    continue;
                }
                facts.push(Fact {
                    time: time_at(row),
                    row_id: chunk.row_ids_slice()[row],
                    chunk: chunk_idx,
                    row,
                    driving_value: None,
                    kind: FactKind::Clear,
                });
            }
        }
    }

    facts
}

/// The latest fact by `(time, RowId)` for each distinct driving value.
fn latest_per_value<'a>(facts: impl IntoIterator<Item = &'a Fact>) -> Vec<&'a Fact> {
    let mut per_value: BTreeMap<&[u8], &Fact> = BTreeMap::new();
    for f in facts {
        let value = f
            .driving_value
            .as_deref()
            .expect("driving facts carry their value");
        let slot = per_value.entry(value).or_insert(f);
        if f.stamp() > slot.stamp() {
            *slot = f;
        }
    }
    per_value.into_values().collect()
}

/// The rows a column shows at exactly `t`, ordered by `RowId`.
fn values_at(facts: &[Fact], t: TimeInt, driving: bool) -> Vec<&Fact> {
    let at_t: Vec<&Fact> = facts.iter().filter(|f| f.time == Some(t)).collect();
    let clear = at_t
        .iter()
        .filter(|f| f.kind == FactKind::Clear)
        .max_by_key(|f| f.row_id)
        .copied();
    let data: Vec<&Fact> = at_t
        .iter()
        .filter(|f| f.kind == FactKind::Data)
        .copied()
        .collect();

    let mut winners: Vec<&Fact> = if driving {
        latest_per_value(data)
    } else {
        data.iter()
            .max_by_key(|f| f.row_id)
            .copied()
            .into_iter()
            .collect()
    };

    if let Some(clear) = clear {
        winners.retain(|f| f.row_id > clear.row_id);
        if winners.is_empty() {
            return vec![clear];
        }
    }
    winners.sort_by_key(|f| f.row_id);
    winners
}

/// The rows a column shows at `t` when it logged nothing there, carried
/// forward.
fn filled_at(facts: &[Fact], t: TimeInt, driving: bool) -> Vec<&Fact> {
    let before: Vec<&Fact> = facts
        .iter()
        .filter(|f| f.time.is_some_and(|ft| ft <= t))
        .collect();
    let clear = before
        .iter()
        .filter(|f| f.kind == FactKind::Clear)
        .max_by_key(|f| f.stamp())
        .copied();
    let clear_stamp = clear.map(Fact::stamp);
    let live = |f: &Fact| f.kind == FactKind::Data && clear_stamp.is_none_or(|c| f.stamp() > c);

    if driving {
        let mut winners = latest_per_value(before.iter().copied().filter(|f| live(f)));
        if let Some(clear) = clear
            && winners.is_empty()
            && clear.time == Some(t)
        {
            return vec![clear];
        }
        winners.sort_by_key(|f| f.row_id);
        return winners;
    }

    before
        .iter()
        .copied()
        .filter(|f| live(f))
        .max_by_key(|f| f.stamp())
        .into_iter()
        .collect()
}

/// Rule 6: per timeline other than `index`, the latest time of the shown
/// rows that carry it.
fn other_times<'a>(
    chunks: &[Arc<Chunk>],
    index: TimelineName,
    shown: impl Iterator<Item = &'a Fact>,
) -> BTreeMap<TimelineName, TimeInt> {
    let mut times: BTreeMap<TimelineName, TimeInt> = BTreeMap::new();
    for fact in shown.filter(|f| f.time.is_some()) {
        for (timeline, column) in chunks[fact.chunk].timelines() {
            if *timeline == index {
                continue;
            }
            let time = TimeInt::new_temporal(column.times_raw()[fact.row]);
            times
                .entry(*timeline)
                .and_modify(|t| *t = (*t).max(time))
                .or_insert(time);
        }
    }
    times
}

/// The rows `query` yields over `chunks`.
pub fn evaluate(chunks: &[Arc<Chunk>], query: &ReferenceQuery) -> Vec<ReferenceRow> {
    let is_driving = |c: usize| query.driving == Some(c);
    let facts: Vec<Vec<Fact>> = query
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| facts_for(chunks, query.index, c, is_driving(i)))
        .collect();
    let static_clear: Vec<Option<RowId>> = facts
        .iter()
        .map(|fs| {
            fs.iter()
                .find(|f| f.time.is_none() && f.kind == FactKind::Clear)
                .map(|f| f.row_id)
        })
        .collect();
    let static_winner: Vec<Option<&Fact>> = std::iter::zip(&facts, &static_clear)
        .map(|(fs, clear)| {
            fs.iter()
                .filter(|f| f.time.is_none() && f.kind == FactKind::Data)
                .filter(|f| clear.is_none_or(|c| f.row_id > c))
                .max_by_key(|f| f.row_id)
        })
        .collect();

    // A row keeps its `RowId` when compaction splits its columns into
    // separate chunks, so a row is matched by `RowId`, not by chunk.
    let in_row = |c: usize, row_id: RowId, kind: FactKind| {
        facts[c]
            .iter()
            .find(|f| f.row_id == row_id && f.kind == kind)
    };

    // A column that some row logged together with the driving column, and
    // that has no static value, reads each output row's value from the row
    // its driving value was logged in. A row belongs to one entity, so a
    // shared `RowId` on another entity (a chunk copied with
    // `Chunk::clone_with_new_entity_path`, say) is not a shared row.
    let follows: Vec<bool> = (0..query.columns.len())
        .map(|c| {
            query.driving.is_some_and(|d| {
                c != d
                    && query.columns[c].entity_path == query.columns[d].entity_path
                    && static_winner[c].is_none()
                    && static_clear[c].is_none()
                    && facts[d].iter().any(|f| {
                        f.kind == FactKind::Data && in_row(c, f.row_id, FactKind::Data).is_some()
                    })
            })
        })
        .collect();

    let grid_source: Vec<usize> = match query.filtered_is_not_null {
        Some(c) => vec![c],
        None => (0..query.columns.len()).collect(),
    };
    let mut grid: Vec<TimeInt> = grid_source
        .iter()
        .flat_map(|&c| {
            let has_static_data = facts[c]
                .iter()
                .any(|f| f.time.is_none() && f.kind == FactKind::Data);
            facts[c]
                .iter()
                .filter(move |f| f.kind == FactKind::Clear || !has_static_data)
                .filter_map(|f| f.time)
        })
        .filter(|t| query.range.is_none_or(|r| r.contains(*t)))
        .collect();
    grid.sort_unstable();
    grid.dedup();

    let mut rows = Vec::new();
    for t in grid {
        let sets: Vec<Vec<&Fact>> = (0..query.columns.len())
            .map(|c| {
                if let Some(s) = static_winner[c] {
                    return vec![s];
                }
                if static_clear[c].is_some() {
                    return Vec::new();
                }
                if query.fill_latest_at && is_driving(c) {
                    return filled_at(&facts[c], t, true);
                }
                let at = values_at(&facts[c], t, is_driving(c));
                if at.is_empty() && query.fill_latest_at {
                    filled_at(&facts[c], t, false)
                } else {
                    at
                }
            })
            .collect();

        let driving_set: &[&Fact] = query.driving.map_or(&[], |d| sets[d].as_slice());

        let multiplicity = driving_set.len().max(1);
        for i in 0..multiplicity {
            let driving_fact = driving_set.get(i).copied();
            let shown: Vec<Option<&Fact>> = sets
                .iter()
                .enumerate()
                .map(|(c, set)| {
                    if is_driving(c) {
                        return driving_fact;
                    }
                    if follows[c]
                        && let Some(driving) = driving_fact
                        && driving.kind == FactKind::Data
                    {
                        // Rule 3: a `Clear` logged in the driving value's row
                        // wins, and like any clear shows only at its own time.
                        if let Some(clear) = in_row(c, driving.row_id, FactKind::Clear) {
                            return (clear.time == Some(t)).then_some(clear);
                        }
                        return in_row(c, driving.row_id, FactKind::Data);
                    }
                    set.first().copied()
                })
                .collect();
            rows.push(ReferenceRow {
                time: t,
                cells: shown
                    .iter()
                    .map(|f| f.map_or(Cell::Null, Fact::cell))
                    .collect(),
                other_times: other_times(chunks, query.index, shown.iter().flatten().copied()),
            });
        }
    }

    rows
}

/// The Arrow value behind a cell: `None` for null, the (possibly empty) list
/// element otherwise. This is what a dataframe batch's list column holds per
/// row, so it compares directly against `QueryHandle` output.
pub fn cell_value(
    chunks: &[Arc<Chunk>],
    column: &ComponentColumnSelector,
    cell: Cell,
) -> Option<ArrayRef> {
    let component = component_of(column);
    match cell {
        Cell::Null => None,
        Cell::Empty => {
            let list = chunks
                .iter()
                .filter(|c| c.entity_path() == &column.entity_path)
                .find_map(|c| c.components().get_array(component))
                .expect("an Empty cell needs a chunk to take the datatype from");
            Some(arrow::array::new_empty_array(&list.value_type()))
        }
        Cell::Value { chunk, row } => Some(
            chunks[chunk]
                .components()
                .get_array(component)
                .expect("a Value cell points at a present component")
                .value(row),
        ),
    }
}

mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use arrow::array::{Array, Int64Array, RecordBatch, RecordBatchOptions};
    use arrow::buffer::NullBuffer;
    use arrow::compute::concat_batches;
    use arrow::datatypes::{DataType, SchemaRef};
    use re_dataframe::external::re_chunk_store::{
        ChunkStore, ChunkStoreConfig, ChunkStoreSubscriber as _,
    };
    use re_dataframe::{QueryCache, QueryEngine, QueryExpression, SparseFillStrategy};
    use re_log_types::{EntityPath, StoreId, StoreKind, TimePoint, TimeType, Timeline};
    use re_sorbet::ColumnDescriptor;
    use re_types_core::ComponentDescriptor;

    use super::super::test_support::{
        TEST_ENTITY, child_frame_component, clear_chunk, column_header, markdown,
        recursive_clear_chunk, render_value, scalar_rows_chunk, sparse_transform_rows_chunk,
        test_component, transform_rows_chunk, translation_component,
    };
    use super::*;

    const INDEX: &str = "frame";

    fn index() -> TimelineName {
        TimelineName::try_new(INDEX).unwrap()
    }

    fn query(columns: Vec<ComponentColumnSelector>) -> ReferenceQuery {
        ReferenceQuery {
            index: index(),
            columns,
            range: None,
            filtered_is_not_null: None,
            fill_latest_at: false,
            driving: None,
        }
    }

    /// Runs `QueryHandle` over the same chunks, as one batch.
    fn query_handle_batch(chunks: &[Arc<Chunk>], q: &ReferenceQuery) -> RecordBatch {
        // The cache only learns about `Clear`s through store events, so a
        // store filled without forwarding them would fill past a clear.
        let store = ChunkStore::new_handle(
            StoreId::random(StoreKind::Recording, "reference_test"),
            ChunkStoreConfig::ALL_DISABLED,
        );
        let cache = QueryCache::new_handle(store.clone());
        for chunk in chunks {
            let events = store.write().insert_chunk(chunk).unwrap();
            cache.write().on_events(&events);
        }
        let engine = QueryEngine::new(store, cache);

        let mut view_contents: BTreeMap<EntityPath, Option<BTreeSet<ComponentIdentifier>>> =
            BTreeMap::new();
        for c in &q.columns {
            view_contents
                .entry(c.entity_path.clone())
                .or_insert_with(|| Some(BTreeSet::new()))
                .as_mut()
                .unwrap()
                .insert(component_of(c));
        }
        let expr = QueryExpression {
            view_contents: Some(re_dataframe::ViewContentsSelector(view_contents)),
            filtered_index: Some(q.index),
            filtered_index_range: q.range,
            filtered_is_not_null: q.filtered_is_not_null.map(|c| q.columns[c].clone()),
            sparse_fill_strategy: if q.fill_latest_at {
                SparseFillStrategy::LatestAtGlobal
            } else {
                SparseFillStrategy::None
            },
            include_tombstone_columns: true,
            ..Default::default()
        };

        let mut handle = engine.query(expr);
        let schema = handle.schema().clone();
        let batches: Vec<RecordBatch> = handle.batch_iter().collect();
        concat_batches(&schema, &batches).unwrap()
    }

    /// [`evaluate`]'s rows as a batch of `schema`, a `QueryHandle` schema:
    /// the index column holds the row times, every other time column the
    /// row's [`ReferenceRow::other_times`], and every component column the
    /// row's cells.
    fn reference_batch(
        chunks: &[Arc<Chunk>],
        q: &ReferenceQuery,
        schema: &SchemaRef,
    ) -> RecordBatch {
        let rows = evaluate(chunks, q);
        let columns: Vec<ArrayRef> = schema
            .fields()
            .iter()
            .map(|field| -> ArrayRef {
                match ColumnDescriptor::try_from_arrow_field(None, field).unwrap() {
                    ColumnDescriptor::RowId(_) => unreachable!("not selected"),
                    ColumnDescriptor::Time(descr) => {
                        let timeline = descr.timeline();
                        let times: Vec<Option<TimeInt>> = rows
                            .iter()
                            .map(|row| {
                                if *timeline.name() == q.index {
                                    Some(row.time)
                                } else {
                                    row.other_times.get(timeline.name()).copied()
                                }
                            })
                            .collect();
                        let values: Vec<i64> =
                            times.iter().map(|t| t.map_or(0, TimeInt::as_i64)).collect();
                        let nulls = NullBuffer::from_iter(times.iter().map(Option::is_some));
                        timeline
                            .typ()
                            .make_arrow_array_with_nulls(values, Some(nulls))
                    }
                    ColumnDescriptor::Component(descr) => {
                        let c = q
                            .columns
                            .iter()
                            .position(|c| {
                                c.entity_path == descr.entity_path
                                    && component_of(c) == descr.component
                            })
                            .unwrap_or_else(|| panic!("{descr:?} is a query column"));
                        let values: Vec<Option<ArrayRef>> = rows
                            .iter()
                            .map(|row| cell_value(chunks, &q.columns[c], row.cells[c]))
                            .collect();
                        let values: Vec<Option<&dyn Array>> =
                            values.iter().map(|v| v.as_deref()).collect();
                        let DataType::List(item) = field.data_type() else {
                            panic!("component columns are lists: {field:?}");
                        };
                        Arc::new(
                            re_arrow_util::arrays_to_list_array(item.data_type().clone(), &values)
                                .expect("cells share the column's datatype"),
                        )
                    }
                }
            })
            .collect();
        let options = RecordBatchOptions::new().with_row_count(Some(rows.len()));
        RecordBatch::try_new_with_options(schema.clone(), columns, &options).unwrap()
    }

    /// The reference's rows and `QueryHandle`'s, both as `QueryHandle`
    /// batches, must be equal; a mismatch prints both as tables.
    fn assert_matches_query_handle(chunks: &[Arc<Chunk>], q: &ReferenceQuery) {
        let actual = query_handle_batch(chunks, q);
        let expected = reference_batch(chunks, q, &actual.schema());
        let (expected_table, actual_table) = (pretty(&expected), pretty(&actual));
        assert!(
            expected_table == actual_table,
            "reference:\n{expected_table}\nQueryHandle:\n{actual_table}"
        );
        assert_eq!(expected, actual);
    }

    fn pretty(batch: &RecordBatch) -> String {
        arrow::util::pretty::pretty_format_batches(std::slice::from_ref(batch))
            .unwrap()
            .to_string()
    }

    fn arcs(chunks: impl IntoIterator<Item = Chunk>) -> Vec<Arc<Chunk>> {
        chunks.into_iter().map(Arc::new).collect()
    }

    /// One row on `entity` at `time` on the index, or static for `None`.
    fn row_chunk(
        entity: &str,
        time: Option<i64>,
        cells: impl IntoIterator<Item = (ComponentDescriptor, ArrayRef)>,
    ) -> Chunk {
        let timepoint = time.map_or_else(
            || TimePoint::STATIC,
            |t| TimePoint::from([(Timeline::new_sequence(INDEX), TimeInt::new_temporal(t))]),
        );
        Chunk::builder(EntityPath::from(entity))
            .with_row(RowId::new(), timepoint, cells)
            .build()
            .unwrap()
    }

    fn ints(values: &[i64]) -> ArrayRef {
        Arc::new(Int64Array::from(values.to_vec()))
    }

    fn column(entity: &str, descr: &ComponentDescriptor) -> ComponentColumnSelector {
        ComponentColumnSelector::from_descriptor(EntityPath::from(entity), descr)
    }

    fn scalar_column() -> ComponentColumnSelector {
        column(TEST_ENTITY, &test_component())
    }

    fn tf_columns() -> (
        ComponentColumnSelector,
        ComponentColumnSelector,
        ComponentColumnSelector,
    ) {
        (
            column("/tf", &child_frame_component()),
            column("/tf", &translation_component()),
            column("/joint", &test_component()),
        )
    }

    /// The example in the module doc, chunks A, B, C and D in order.
    fn module_doc_example_chunks() -> Vec<Arc<Chunk>> {
        arcs([
            transform_rows_chunk("/tf", INDEX, &[(20, "a", 1), (20, "b", 2)]),
            scalar_rows_chunk("/joint", INDEX, &[(10, 1), (20, 2), (30, 3), (40, 4)]),
            transform_rows_chunk("/tf", INDEX, &[(20, "c", 3)]),
            transform_rows_chunk("/tf", INDEX, &[(40, "a", 11)]),
        ])
    }

    /// The reference's output for `q` as the module doc's markdown table: the
    /// index, then a column per selected column, then a column per other
    /// timeline in `chunks`; strings in backticks, a `Clear` as `[]`.
    fn table(chunks: &[Arc<Chunk>], q: &ReferenceQuery) -> String {
        let timelines: BTreeSet<TimelineName> = chunks
            .iter()
            .flat_map(|chunk| chunk.timelines().keys().copied())
            .filter(|timeline| *timeline != q.index)
            .collect();
        let mut header = vec![format!("`{}`", q.index)];
        header.extend(q.columns.iter().map(column_header));
        header.extend(timelines.iter().map(|timeline| format!("`{timeline}`")));
        let body: Vec<Vec<String>> = evaluate(chunks, q)
            .iter()
            .map(|row| {
                let mut cells = vec![row.time.as_i64().to_string()];
                cells.extend(
                    std::iter::zip(&q.columns, &row.cells)
                        .map(|(c, cell)| render_value(cell_value(chunks, c, *cell))),
                );
                cells.extend(timelines.iter().map(|timeline| {
                    row.other_times
                        .get(timeline)
                        .map_or_else(|| "null".to_owned(), |t| t.as_i64().to_string())
                }));
                cells
            })
            .collect();
        markdown(&header, &body)
    }

    /// Overlapping chunks, a cross-chunk collision, a same-chunk run, a static
    /// shadow on a second component, a second entity, a flat clear and a chunk
    /// on another timeline: with no driving column, the output must match
    /// `QueryHandle` with and without fill. At 30 the later chunk's 33 corrects
    /// 3 though it is listed first, since `RowId`s decide, not chunk order; at
    /// 70 the run keeps its last row, 73.
    #[test]
    fn matches_query_handle_on_nasty_shapes() {
        let other = ComponentDescriptor::partial("other");
        let early = scalar_rows_chunk(
            TEST_ENTITY,
            INDEX,
            &[(10, 1), (30, 3), (50, 5), (70, 71), (70, 72), (70, 73)],
        );
        let late = scalar_rows_chunk(TEST_ENTITY, INDEX, &[(20, 2), (30, 33), (40, 4)]);
        let chunks = arcs([
            late,
            early,
            row_chunk(TEST_ENTITY, None, [(other.clone(), ints(&[99]))]),
            scalar_rows_chunk("/other", INDEX, &[(10, 100), (40, 400)]),
            clear_chunk(TEST_ENTITY, INDEX, Some(35)),
            scalar_rows_chunk(TEST_ENTITY, "other_timeline", &[(25, 999)]),
        ]);
        let columns = vec![
            scalar_column(),
            column(TEST_ENTITY, &other),
            column("/other", &test_component()),
        ];

        let mut q = query(columns);
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:value` | `/points:other` | `/other:value` | `other_timeline` |
        |---------|-----------------|-----------------|----------------|------------------|
        | 10      | 1               | 99              | 100            | null             |
        | 20      | 2               | 99              | null           | null             |
        | 30      | 33              | 99              | null           | null             |
        | 35      | []              | 99              | null           | null             |
        | 40      | 4               | 99              | 400            | null             |
        | 50      | 5               | 99              | null           | null             |
        | 70      | 73              | 99              | null           | null             |
        ");

        q.fill_latest_at = true;
        assert_matches_query_handle(&chunks, &q);

        q.filtered_is_not_null = Some(2);
        assert_matches_query_handle(&chunks, &q);

        q.filtered_is_not_null = None;
        q.range = Some(AbsoluteTimeRange::new(20, 40));
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:value` | `/points:other` | `/other:value` | `other_timeline` |
        |---------|-----------------|-----------------|----------------|------------------|
        | 20      | 2               | 99              | 100            | null             |
        | 30      | 33              | 99              | 100            | null             |
        | 35      | []              | 99              | 100            | null             |
        | 40      | 4               | 99              | 400            | null             |
        ");
    }

    /// A static value shadows the column's temporal rows whatever their
    /// `RowId`s, the latest static value wins, and the shadowed rows add no
    /// times to the grid.
    #[test]
    fn static_shadows_temporal_on_its_column() {
        let first = row_chunk(TEST_ENTITY, None, [(test_component(), ints(&[99]))]);
        let second = row_chunk(TEST_ENTITY, None, [(test_component(), ints(&[77]))]);
        let temporal = scalar_rows_chunk(TEST_ENTITY, INDEX, &[(10, 1), (30, 3)]);
        let chunks = arcs([
            temporal,
            second,
            first,
            scalar_rows_chunk("/other", INDEX, &[(20, 2)]),
        ]);

        let mut only = query(vec![scalar_column()]);
        assert_matches_query_handle(&chunks, &only);
        insta::assert_snapshot!(table(&chunks, &only), @r"
        | `frame` | `/points:value` |
        |---------|-----------------|
        ");
        only.fill_latest_at = true;
        assert_matches_query_handle(&chunks, &only);

        let mut both = query(vec![scalar_column(), column("/other", &test_component())]);
        assert_matches_query_handle(&chunks, &both);
        let without_fill = table(&chunks, &both);
        insta::assert_snapshot!(without_fill, @r"
        | `frame` | `/points:value` | `/other:value` |
        |---------|-----------------|----------------|
        | 20      | 77              | 2              |
        ");
        both.fill_latest_at = true;
        assert_matches_query_handle(&chunks, &both);
        assert_eq!(
            table(&chunks, &both),
            without_fill,
            "fill adds nothing here"
        );
    }

    /// A temporal `Clear` on the entity of a column with a static value still
    /// creates a row, which shows the static value; the shadowed temporal
    /// value does not.
    #[test]
    fn temporal_clear_on_a_static_column_creates_a_row() {
        let chunks = arcs([
            scalar_rows_chunk(TEST_ENTITY, INDEX, &[(10, 1)]),
            row_chunk(TEST_ENTITY, None, [(test_component(), ints(&[99]))]),
            clear_chunk(TEST_ENTITY, INDEX, Some(20)),
            scalar_rows_chunk("/other", INDEX, &[(40, 4)]),
        ]);
        let mut q = query(vec![scalar_column(), column("/other", &test_component())]);
        assert_matches_query_handle(&chunks, &q);
        let without_fill = table(&chunks, &q);
        insta::assert_snapshot!(without_fill, @r"
        | `frame` | `/points:value` | `/other:value` |
        |---------|-----------------|----------------|
        | 20      | 99              | null           |
        | 40      | 99              | 4              |
        ");
        q.fill_latest_at = true;
        assert_matches_query_handle(&chunks, &q);
        assert_eq!(table(&chunks, &q), without_fill, "fill adds nothing here");

        q.filtered_is_not_null = Some(0);
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:value` | `/other:value` |
        |---------|-----------------|----------------|
        | 20      | 99              | null           |
        ");
    }

    /// A static `Clear` on an entity shadows its temporal `Clear`s: they add
    /// no times.
    #[test]
    fn static_clear_shadows_temporal_clears_on_its_entity() {
        let chunks = arcs([
            scalar_rows_chunk(TEST_ENTITY, INDEX, &[(10, 1)]),
            clear_chunk(TEST_ENTITY, INDEX, None),
            clear_chunk(TEST_ENTITY, INDEX, Some(20)),
            scalar_rows_chunk("/other", INDEX, &[(40, 4)]),
        ]);
        let q = query(vec![scalar_column(), column("/other", &test_component())]);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:value` | `/other:value` |
        |---------|-----------------|----------------|
        | 10      | null            | null           |
        | 40      | null            | 4              |
        ");
        // `QueryHandle` differs here: see the exceptions in the module doc.
        insta::assert_snapshot!(pretty(&query_handle_batch(&chunks, &q)), @r"
        +-------+--------------+---------------+
        | frame | /other:value | /points:value |
        +-------+--------------+---------------+
        | 10    |              | [1]           |
        | 40    | [4]          |               |
        +-------+--------------+---------------+
        ");
    }

    /// A driving value is a whole logged cell: a row logging two child frames
    /// in one batch is one value and one row, apart from a row logging one of
    /// them alone.
    #[test]
    fn a_batch_of_instances_is_one_driving_value() {
        let (child_frame, translation, _) = tf_columns();
        let frames: ArrayRef = Arc::new(arrow::array::StringArray::from(vec!["a", "b"]));
        let chunks = arcs([
            row_chunk(
                "/tf",
                Some(20),
                [
                    (child_frame_component(), frames),
                    (translation_component(), ints(&[1, 2])),
                ],
            ),
            transform_rows_chunk("/tf", INDEX, &[(20, "a", 3)]),
        ]);
        let mut q = query(vec![child_frame, translation]);
        q.driving = Some(0);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | [`a`, `b`]        | [1, 2]            |
        | 20      | `a`               | 3                 |
        ");
    }

    /// A following column is null on a row whose driving value was logged
    /// without it, even if it logged its own row at that time or the driving
    /// value had it before a correction, and shows its own value at a time the
    /// driving column has nothing.
    #[test]
    fn following_column_is_null_when_its_row_did_not_log_it() {
        let (child_frame, translation, _) = tf_columns();
        let mut chunks = arcs([sparse_transform_rows_chunk(
            "/tf",
            INDEX,
            &[
                (20, Some("a"), Some(1)),
                (20, Some("b"), None),
                (20, None, Some(5)),
                (30, None, Some(7)),
            ],
        )]);
        let mut q = query(vec![child_frame, translation]);
        q.driving = Some(0);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | `a`               | 1                 |
        | 20      | `b`               | null              |
        | 30      | null              | 7                 |
        ");

        chunks.push(Arc::new(sparse_transform_rows_chunk(
            "/tf",
            INDEX,
            &[(20, Some("a"), None)],
        )));
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | `b`               | null              |
        | 20      | `a`               | null              |
        | 30      | null              | 7                 |
        ");
    }

    /// `filtered_is_not_null` on a following column picks the times only: a
    /// driving value logged without that column still gets its row, null in
    /// it.
    #[test]
    fn not_null_on_a_following_column_keeps_every_driving_row() {
        let (child_frame, translation, _) = tf_columns();
        let chunks = arcs([sparse_transform_rows_chunk(
            "/tf",
            INDEX,
            &[
                (20, Some("a"), Some(1)),
                (20, Some("b"), None),
                (30, Some("c"), None),
            ],
        )]);
        let mut q = query(vec![child_frame, translation]);
        q.driving = Some(0);
        q.filtered_is_not_null = Some(1);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | `a`               | 1                 |
        | 20      | `b`               | null              |
        ");
    }

    /// A column with a static value repeats it next to every driving value,
    /// even though its temporal rows were logged with the driving column.
    #[test]
    fn static_value_does_not_follow_the_driving_column() {
        let (child_frame, translation, _) = tf_columns();
        let chunks = arcs([
            transform_rows_chunk("/tf", INDEX, &[(20, "a", 1), (20, "b", 2)]),
            row_chunk("/tf", None, [(translation_component(), ints(&[99]))]),
        ]);
        let mut q = query(vec![child_frame, translation]);
        q.driving = Some(0);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | `a`               | 99                |
        | 20      | `b`               | 99                |
        ");
    }

    /// At the clear's own time, data with a higher `RowId` than the clear
    /// survives and data with a lower one is cleared.
    #[test]
    fn clear_competes_by_row_id_at_its_own_time() {
        let chunks = arcs([
            scalar_rows_chunk("/a", INDEX, &[(20, 1)]),
            scalar_rows_chunk("/b", INDEX, &[(20, 1)]),
            clear_chunk("/a", INDEX, Some(20)),
            clear_chunk("/b", INDEX, Some(20)),
            scalar_rows_chunk("/a", INDEX, &[(20, 3)]),
        ]);
        let q = query(vec![
            column("/a", &test_component()),
            column("/b", &test_component()),
        ]);
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/a:value` | `/b:value` |
        |---------|------------|------------|
        | 20      | 3          | []         |
        ");
    }

    /// A `Clear` on an ancestor never reaches the child, recursive or not.
    #[test]
    fn clear_on_an_ancestor_does_not_apply() {
        let flat = arcs([
            scalar_rows_chunk("/parent/child", INDEX, &[(20, 1)]),
            clear_chunk("/parent", INDEX, Some(20)),
            clear_chunk("/parent", INDEX, Some(30)),
            clear_chunk("/parent", INDEX, None),
        ]);
        let mut q = query(vec![column("/parent/child", &test_component())]);
        q.fill_latest_at = true;
        assert_matches_query_handle(&flat, &q);
        let expected = table(&flat, &q);
        insta::assert_snapshot!(expected, @r"
        | `frame` | `/parent/child:value` |
        |---------|-----------------------|
        | 20      | 1                     |
        ");

        // `QueryHandle` differs here: see the exceptions in the module doc.
        let recursive = arcs([
            scalar_rows_chunk("/parent/child", INDEX, &[(20, 1)]),
            recursive_clear_chunk("/parent", INDEX, Some(20)),
            recursive_clear_chunk("/parent", INDEX, Some(30)),
            recursive_clear_chunk("/", INDEX, None),
        ]);
        assert_eq!(table(&recursive, &q), expected);
    }

    /// A `Clear`'s own column shows its flag: at its time and filled forward
    /// for a temporal `Clear`, at every time for a static one.
    #[test]
    fn clear_does_not_clear_its_own_column() {
        let flag = column("/e", &archetypes::Clear::descriptor_is_recursive());
        let temporal = arcs([
            scalar_rows_chunk("/e", INDEX, &[(10, 1), (30, 3)]),
            recursive_clear_chunk("/e", INDEX, Some(20)),
        ]);
        let mut q = query(vec![column("/e", &test_component()), flag.clone()]);
        assert_matches_query_handle(&temporal, &q);
        insta::assert_snapshot!(table(&temporal, &q), @r"
        | `frame` | `/e:value` | `/e:Clear:is_recursive` |
        |---------|------------|-------------------------|
        | 10      | 1          | null                    |
        | 20      | []         | true                    |
        | 30      | 3          | null                    |
        ");
        q.fill_latest_at = true;
        assert_matches_query_handle(&temporal, &q);
        insta::assert_snapshot!(table(&temporal, &q), @r"
        | `frame` | `/e:value` | `/e:Clear:is_recursive` |
        |---------|------------|-------------------------|
        | 10      | 1          | null                    |
        | 20      | []         | true                    |
        | 30      | 3          | true                    |
        ");

        // `/e:value` is left out: `QueryHandle` differs on it, see the
        // exceptions in the module doc.
        let statics = arcs([
            scalar_rows_chunk("/e", INDEX, &[(10, 1)]),
            clear_chunk("/e", INDEX, None),
            scalar_rows_chunk("/other", INDEX, &[(10, 1), (30, 3)]),
        ]);
        let q = query(vec![flag, column("/other", &test_component())]);
        assert_matches_query_handle(&statics, &q);
        insta::assert_snapshot!(table(&statics, &q), @r"
        | `frame` | `/e:Clear:is_recursive` | `/other:value` |
        |---------|-------------------------|----------------|
        | 10      | false                   | 1              |
        | 30      | false                   | 3              |
        ");
    }

    /// A value and a `Clear` logged in one row share its `RowId`; the `Clear`
    /// wins, at that time and for a later fill.
    #[test]
    fn clear_wins_over_a_value_in_the_same_row() {
        let flag: ArrayRef = Arc::new(arrow::array::BooleanArray::from(vec![false]));
        let both = row_chunk(
            TEST_ENTITY,
            Some(20),
            [
                (test_component(), ints(&[1])),
                (archetypes::Clear::descriptor_is_recursive(), flag),
            ],
        );
        let chunks = arcs([both, scalar_rows_chunk("/other", INDEX, &[(30, 3)])]);
        let mut q = query(vec![scalar_column(), column("/other", &test_component())]);
        q.fill_latest_at = true;
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:value` | `/other:value` |
        |---------|-----------------|----------------|
        | 20      | []              | null           |
        | 30      | null            | 3              |
        ");
        // `QueryHandle` differs here: see the exceptions in the module doc.
        insta::assert_snapshot!(pretty(&query_handle_batch(&chunks, &q)), @r"
        +-------+--------------+---------------+
        | frame | /other:value | /points:value |
        +-------+--------------+---------------+
        | 20    |              | [1]           |
        | 30    | [3]          |               |
        +-------+--------------+---------------+
        ");

        // The same with the `Clear`'s own column, which is never cleared, as
        // the driving column: the value logged in its row still follows rule 3.
        let mut q = query(vec![
            column(TEST_ENTITY, &archetypes::Clear::descriptor_is_recursive()),
            scalar_column(),
            column("/other", &test_component()),
        ]);
        q.driving = Some(0);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:Clear:is_recursive` | `/points:value` | `/other:value` |
        |---------|------------------------------|-----------------|----------------|
        | 20      | false                        | []              | null           |
        | 30      | null                         | null            | 3              |
        ");
        q.fill_latest_at = true;
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:Clear:is_recursive` | `/points:value` | `/other:value` |
        |---------|------------------------------|-----------------|----------------|
        | 20      | false                        | []              | null           |
        | 30      | false                        | null            | 3              |
        ");
    }

    /// The `log_time` timeline of the rule 6 fixtures.
    const LOG_TIME: &str = "log_time";

    /// Rows on `/joints` and `/camera`, each on the index and on `log_time`,
    /// with the index of type `index_type` and `log_time` of type
    /// `other_type`, a temporal `Clear` on `/camera` and a static value on
    /// [`TEST_ENTITY`]. The joints at frame 10, logged at 100, are corrected
    /// in the same frame by a row logged at 99.
    fn log_time_chunks(index_type: TimeType, other_type: TimeType) -> Vec<Arc<Chunk>> {
        let timepoint = |frame: i64, other: i64| {
            TimePoint::from([
                (
                    Timeline::new(INDEX, index_type),
                    TimeInt::new_temporal(frame),
                ),
                (
                    Timeline::new(LOG_TIME, other_type),
                    TimeInt::new_temporal(other),
                ),
            ])
        };
        let rows = |entity: &str, rows: &[(i64, i64, i64)]| {
            let mut builder = Chunk::builder(EntityPath::from(entity));
            for &(frame, other, value) in rows {
                let value: ArrayRef = Arc::new(Int64Array::from(vec![value]));
                builder = builder.with_row(
                    RowId::new(),
                    timepoint(frame, other),
                    [(test_component(), value)],
                );
            }
            builder.build().unwrap()
        };
        let flag: ArrayRef = Arc::new(arrow::array::BooleanArray::from(vec![false]));
        let camera_clear = Chunk::builder(EntityPath::from("/camera"))
            .with_row(
                RowId::new(),
                timepoint(40, 500),
                [(archetypes::Clear::descriptor_is_recursive(), flag)],
            )
            .build()
            .unwrap();
        arcs([
            rows("/joints", &[(10, 100, 1), (30, 300, 5)]),
            rows("/joints", &[(10, 99, 3)]),
            rows("/camera", &[(20, 400, 4)]),
            camera_clear,
            row_chunk(TEST_ENTITY, None, [(test_component(), ints(&[7]))]),
        ])
    }

    fn log_time_query() -> ReferenceQuery {
        query(vec![
            column("/joints", &test_component()),
            column("/camera", &test_component()),
            scalar_column(),
        ])
    }

    /// Rule 6: the other timeline is the latest of the rows the cells show,
    /// a `Clear` and a filled value included.
    #[test]
    fn other_timeline_is_the_latest_of_the_shown_rows() {
        let chunks = log_time_chunks(TimeType::Sequence, TimeType::Sequence);
        let mut q = log_time_query();
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/joints:value` | `/camera:value` | `/points:value` | `log_time` |
        |---------|-----------------|-----------------|-----------------|------------|
        | 10      | 3               | null            | 7               | 99         |
        | 20      | null            | 4               | 7               | 400        |
        | 30      | 5               | null            | 7               | 300        |
        | 40      | null            | []              | 7               | 500        |
        ");

        q.fill_latest_at = true;
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/joints:value` | `/camera:value` | `/points:value` | `log_time` |
        |---------|-----------------|-----------------|-----------------|------------|
        | 10      | 3               | null            | 7               | 99         |
        | 20      | 3               | 4               | 7               | 400        |
        | 30      | 5               | 4               | 7               | 400        |
        | 40      | 5               | []              | 7               | 500        |
        ");

        // `QueryHandle` differs here: see the exceptions in the module doc.
        let mut hidden = chunks.clone();
        hidden.push(Arc::new(clear_chunk("/camera", INDEX, None)));
        insta::assert_snapshot!(table(&hidden, &q), @r"
        | `frame` | `/joints:value` | `/camera:value` | `/points:value` | `log_time` |
        |---------|-----------------|-----------------|-----------------|------------|
        | 10      | 3               | null            | 7               | 99         |
        | 20      | 3               | null            | 7               | 99         |
        | 30      | 5               | null            | 7               | 300        |
        ");
    }

    /// Timestamp and duration timelines, as the index or as another
    /// timeline, give the same rows as sequence timelines, and `QueryHandle`
    /// agrees on them.
    #[test]
    fn every_time_type_gives_the_same_rows() {
        let mut q = log_time_query();
        for fill_latest_at in [false, true] {
            q.fill_latest_at = fill_latest_at;
            let sequence = table(&log_time_chunks(TimeType::Sequence, TimeType::Sequence), &q);
            for (index_type, other_type) in [
                (TimeType::TimestampNs, TimeType::DurationNs),
                (TimeType::DurationNs, TimeType::TimestampNs),
            ] {
                let chunks = log_time_chunks(index_type, other_type);
                assert_matches_query_handle(&chunks, &q);
                assert_eq!(
                    table(&chunks, &q),
                    sequence,
                    "index {index_type:?}, {LOG_TIME} {other_type:?}, fill {fill_latest_at}"
                );
            }
        }
    }

    /// A column on another entity never follows the driving column, even
    /// when its rows share `RowId`s with it.
    #[test]
    fn a_copy_on_another_entity_does_not_follow() {
        let (child_frame, translation, _) = tf_columns();
        let transforms = transform_rows_chunk("/tf", INDEX, &[(20, "a", 1), (20, "b", 2)]);
        let copy = transforms.clone_with_new_entity_path(EntityPath::from("/copy"));
        let mut q = query(vec![
            child_frame,
            translation,
            column("/copy", &translation_component()),
        ]);
        q.driving = Some(0);
        insta::assert_snapshot!(table(&arcs([transforms, copy]), &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` | `/copy:translation` |
        |---------|-------------------|-------------------|---------------------|
        | 20      | `a`               | 1                 | 2                   |
        | 20      | `b`               | 2                 | 2                   |
        ");
    }

    /// In the driving column, a clear at `t` removes the values logged before
    /// it and leaves those logged after it; with nothing after it the time is
    /// one empty row.
    #[test]
    fn clear_in_the_driving_column_competes_per_value() {
        let (child_frame, translation, _) = tf_columns();
        let mut chunks = module_doc_example_chunks();
        chunks.push(Arc::new(clear_chunk("/tf", INDEX, Some(20))));
        let mut q = query(vec![child_frame, translation]);
        q.driving = Some(0);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | []                | []                |
        | 40      | `a`               | 11                |
        ");

        chunks.push(Arc::new(transform_rows_chunk(
            "/tf",
            INDEX,
            &[(20, "d", 4)],
        )));
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | `d`               | 4                 |
        | 40      | `a`               | 11                |
        ");
    }

    /// A static `Clear` on each entity hides every temporal value and the
    /// static value logged before it, at every time and with or without fill;
    /// a static value logged afterwards shows. The cleared static value still
    /// shadows its column's temporal times: 50 makes no row.
    #[test]
    fn static_clear_applies_at_every_time() {
        let before = ComponentDescriptor::partial("before");
        let after = ComponentDescriptor::partial("after");
        let chunks = arcs([
            scalar_rows_chunk(TEST_ENTITY, INDEX, &[(10, 1), (30, 3)]),
            scalar_rows_chunk("/other", INDEX, &[(20, 2)]),
            row_chunk(TEST_ENTITY, Some(50), [(before.clone(), ints(&[5]))]),
            row_chunk(TEST_ENTITY, None, [(before.clone(), ints(&[99]))]),
            clear_chunk(TEST_ENTITY, INDEX, None),
            clear_chunk("/other", INDEX, None),
            clear_chunk("/grid", INDEX, None),
            row_chunk(TEST_ENTITY, None, [(after.clone(), ints(&[77]))]),
            scalar_rows_chunk("/grid", INDEX, &[(40, 4)]),
        ]);
        let mut q = query(vec![
            scalar_column(),
            column(TEST_ENTITY, &before),
            column(TEST_ENTITY, &after),
            column("/other", &test_component()),
            column("/grid", &test_component()),
        ]);

        let without_fill = table(&chunks, &q);
        insta::assert_snapshot!(without_fill, @r"
        | `frame` | `/points:value` | `/points:before` | `/points:after` | `/other:value` | `/grid:value` |
        |---------|-----------------|------------------|-----------------|----------------|---------------|
        | 10      | null            | null             | 77              | null           | null          |
        | 20      | null            | null             | 77              | null           | null          |
        | 30      | null            | null             | 77              | null           | null          |
        | 40      | null            | null             | 77              | null           | null          |
        ");
        q.fill_latest_at = true;
        assert_eq!(table(&chunks, &q), without_fill, "fill adds nothing here");

        // `QueryHandle` differs here: see the exceptions in the module doc.
        let q = query(vec![scalar_column()]);
        insta::assert_snapshot!(pretty(&query_handle_batch(&chunks, &q)), @r"
        +-------+---------------+
        | frame | /points:value |
        +-------+---------------+
        | 10    | [1]           |
        | 30    | [3]           |
        +-------+---------------+
        ");
    }

    /// The module doc's output table, with `/tf:child_frame` as the driving
    /// column, must appear in the doc verbatim, so the doc cannot drift from
    /// what the reference computes. Without a driving column, chunk C reads as
    /// a correction of chunk A, as rule 2 says.
    #[test]
    fn module_doc_example() {
        let doc = include_str!("reference.rs");
        let chunks = module_doc_example_chunks();
        let (child_frame, translation, joint) = tf_columns();
        let mut q = query(vec![child_frame, translation, joint]);
        q.driving = Some(0);

        let as_doc: Vec<String> = table(&chunks, &q)
            .lines()
            .map(|line| format!("//! {line}"))
            .collect();
        let as_doc = as_doc.join("\n");
        assert!(
            doc.contains(&as_doc),
            "the module doc must contain this table verbatim:\n{as_doc}"
        );

        q.driving = None;
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` | `/joint:value` |
        |---------|-------------------|-------------------|----------------|
        | 10      | null              | null              | 1              |
        | 20      | `c`               | 3                 | 2              |
        | 30      | null              | null              | 3              |
        | 40      | `a`               | 11                | 4              |
        ");
    }

    /// With fill, each driving value keeps its latest row: chunk D updates
    /// `a` at 40 while `b` and `c` carry forward from 20.
    #[test]
    fn module_doc_example_with_fill() {
        let chunks = module_doc_example_chunks();
        let (child_frame, translation, joint) = tf_columns();
        let mut q = query(vec![child_frame, translation, joint]);
        q.driving = Some(0);
        q.fill_latest_at = true;
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` | `/joint:value` |
        |---------|-------------------|-------------------|----------------|
        | 10      | null              | null              | 1              |
        | 20      | `a`               | 1                 | 2              |
        | 20      | `b`               | 2                 | 2              |
        | 20      | `c`               | 3                 | 2              |
        | 30      | `a`               | 1                 | 3              |
        | 30      | `b`               | 2                 | 3              |
        | 30      | `c`               | 3                 | 3              |
        | 40      | `b`               | 2                 | 4              |
        | 40      | `c`               | 3                 | 4              |
        | 40      | `a`               | 11                | 4              |
        ");
    }

    /// With fill, a clear on the driving column's entity shows as empty at its
    /// own time and ends the carry-forward, as it does for any other column.
    #[test]
    fn driving_fill_respects_a_clear() {
        let (child_frame, translation, joint) = tf_columns();
        let mut chunks = module_doc_example_chunks();
        chunks.push(Arc::new(clear_chunk("/tf", INDEX, Some(30))));
        chunks.push(Arc::new(clear_chunk("/joint", INDEX, Some(30))));

        let mut q = query(vec![child_frame, translation, joint]);
        q.driving = Some(0);
        q.fill_latest_at = true;
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` | `/joint:value` |
        |---------|-------------------|-------------------|----------------|
        | 10      | null              | null              | 1              |
        | 20      | `a`               | 1                 | 2              |
        | 20      | `b`               | 2                 | 2              |
        | 20      | `c`               | 3                 | 2              |
        | 30      | []                | []                | []             |
        | 40      | `a`               | 11                | 4              |
        ");
    }

    /// Equal driving values at one time are one thing logged twice, whichever
    /// chunks they are in: the later row wins, and its other columns with it.
    #[test]
    fn equal_driving_values_are_a_correction() {
        let (child_frame, translation, _) = tf_columns();
        let mut q = query(vec![child_frame, translation]);
        q.driving = Some(0);

        let mut chunks = module_doc_example_chunks();
        chunks.push(Arc::new(transform_rows_chunk(
            "/tf",
            INDEX,
            &[(20, "b", 22)],
        )));
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` |
        |---------|-------------------|-------------------|
        | 20      | `a`               | 1                 |
        | 20      | `c`               | 3                 |
        | 20      | `b`               | 22                |
        | 40      | `a`               | 11                |
        ");
    }

    /// A column on the driving column's entity that is logged in separate
    /// rows does not follow the driving column: it keeps its latest row at
    /// the time and repeats it. Neither merging its chunk into
    /// the driving column's nor splitting the driving column's rows into one
    /// chunk per column, as compaction may do, changes anything.
    #[test]
    fn columns_follow_by_shared_rows_not_by_chunk() {
        let (child_frame, translation, _) = tf_columns();
        let quality = column("/tf", &test_component());
        let transforms = transform_rows_chunk("/tf", INDEX, &[(20, "a", 1), (20, "b", 2)]);
        let scalars = scalar_rows_chunk("/tf", INDEX, &[(20, 8), (20, 9)]);
        let merged = Chunk::concat_and_sort(&transforms, &scalars).unwrap();
        let frames_only = transforms.components_sliced(&[child_frame_component().component]);
        let translations_only = transforms.components_sliced(&[translation_component().component]);

        let mut q = query(vec![child_frame, translation, quality]);
        q.driving = Some(0);
        let separate = table(&arcs([transforms, scalars.clone()]), &q);
        insta::assert_snapshot!(separate, @r"
        | `frame` | `/tf:child_frame` | `/tf:translation` | `/tf:value` |
        |---------|-------------------|-------------------|-------------|
        | 20      | `a`               | 1                 | 9           |
        | 20      | `b`               | 2                 | 9           |
        ");
        assert_eq!(table(&arcs([merged]), &q), separate, "merged chunk");
        assert_eq!(
            table(&arcs([translations_only, frames_only, scalars]), &q),
            separate,
            "one chunk per column"
        );
    }

    /// A clear before the column's first value, listed before any chunk that
    /// carries the column, is an empty list of the column's type.
    #[test]
    fn clear_before_any_data_is_empty() {
        let chunks = arcs([
            clear_chunk(TEST_ENTITY, INDEX, Some(10)),
            scalar_rows_chunk(TEST_ENTITY, INDEX, &[(20, 2)]),
        ]);
        let q = query(vec![scalar_column()]);
        assert_matches_query_handle(&chunks, &q);
        insta::assert_snapshot!(table(&chunks, &q), @r"
        | `frame` | `/points:value` |
        |---------|-----------------|
        | 10      | []              |
        | 20      | 2               |
        ");
    }
}
