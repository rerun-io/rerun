use std::collections::BTreeMap;

use re_chunk_store::{LatestAtQueryExpression, RangeQueryExpression};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QueryExpression {
    LatestAt(re_chunk_store::LatestAtQueryExpression),
    Range(re_chunk_store::RangeQueryExpression),
}

impl From<re_chunk_store::LatestAtQueryExpression> for QueryExpression {
    fn from(value: LatestAtQueryExpression) -> Self {
        Self::LatestAt(value)
    }
}

impl From<re_chunk_store::RangeQueryExpression> for QueryExpression {
    fn from(value: RangeQueryExpression) -> Self {
        Self::Range(value)
    }
}

/// Storage for [`ExpandedRows`], which should be persisted across frames.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExpandedRowsCache {
    /// Maps "table row number" to "additional expanded rows".
    ///
    /// When expanded, the base space is still used for the summary, which the additional space is
    /// used for instances.
    expanded_rows: BTreeMap<u64, u64>,

    /// Keep track of the query for which this cache is valid.
    // TODO(ab): is there a better invalidation strategy? This doesn't capture the fact that the
    // returned data might vary with time (e.g. upon ingestion)
    valid_for: Option<QueryExpression>,
}

impl ExpandedRowsCache {
    /// This sets the query used for cache invalidation.
    ///
    /// If the query doesn't match the cached one, the state will be reset.
    fn set_query(&mut self, query_expression: QueryExpression) {
        if Some(&query_expression) != self.valid_for.as_ref() {
            self.valid_for = Some(query_expression);
            self.expanded_rows = BTreeMap::default();
        }
    }
}

/// Helper to keep track of row expansion.
///
/// This is a short-lived struct to be created every frame. The persistent state is stored in
/// [`ExpandedRowsCache`].
pub(crate) struct ExpandedRows<'a> {
    /// Base row height.
    row_height: f32,

    /// Cache containing the row expanded-ness.
    cache: &'a mut ExpandedRowsCache,
}

impl<'a> ExpandedRows<'a> {
    pub(crate) fn new(
        cache: &'a mut ExpandedRowsCache,
        query_expression: impl Into<QueryExpression>,
        row_height: f32,
    ) -> Self {
        // validate the cache
        cache.set_query(query_expression.into());

        Self { row_height, cache }
    }

    /// Implementation for [`egui_table::Table::row_top_offset`].
    pub(crate) fn row_top_offset(
        &self,
        ctx: &egui::Context,
        table_id: egui::Id,
        row_nr: u64,
    ) -> f32 {
        self.cache
            .expanded_rows
            .range(0..row_nr)
            .map(|(expanded_row_nr, expanded)| {
                let how_expanded = ctx.animate_bool(table_id.with(expanded_row_nr), *expanded > 0);
                how_expanded * *expanded as f32 * self.row_height
            })
            .sum::<f32>()
            + row_nr as f32 * self.row_height
    }

    pub(crate) fn collapse_row(&mut self, row_nr: u64) {
        self.expand_row(row_nr, 0);
    }

    pub(crate) fn expand_row(&mut self, row_nr: u64, additional_row_space: u64) {
        if additional_row_space == 0 {
            self.cache.expanded_rows.remove(&row_nr);
        } else {
            self.cache
                .expanded_rows
                .insert(row_nr, additional_row_space);
        }
    }

    /// Return by how much row space this row is expended.
    pub(crate) fn row_expansion(&self, row_nr: u64) -> u64 {
        self.cache.expanded_rows.get(&row_nr).copied().unwrap_or(0)
    }
}
