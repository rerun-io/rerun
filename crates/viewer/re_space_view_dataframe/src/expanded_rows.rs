use std::collections::BTreeMap;

use re_chunk_store::{LatestAtQueryExpression, RangeQueryExpression};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
///
/// Uses egui's animation support to animate the row expansion/contraction. For this to work:
/// - When collapsed, the row entry must be set to 0 instead of being removed. Otherwise, it will no
///   longer be "seen" by the animation code. Technically, it could be removed _after_ the
///   animation completes, but it's not worth the complexity.
/// - When the row is first expanded, for the animation to work, it must be immediately seeded to 0
///   for the animation to have a starting point.
pub(crate) struct ExpandedRows<'a> {
    /// Base row height.
    row_height: f32,

    /// Cache containing the row expanded-ness.
    cache: &'a mut ExpandedRowsCache,

    /// [`egui::Context`] used to animate the row expansion.
    egui_ctx: egui::Context,

    /// [`egui::Id`] used to store the animation state.
    id: egui::Id,
}

impl<'a> ExpandedRows<'a> {
    /// Create a new [`ExpandedRows`] instance.
    ///
    /// `egui_ctx` is used to animate the row expansion
    /// `id` is used to store the animation state, make it persistent and unique
    /// `query_expression` is used to invalidate the cache
    pub(crate) fn new(
        egui_ctx: egui::Context,
        id: egui::Id,
        cache: &'a mut ExpandedRowsCache,
        query_expression: impl Into<QueryExpression>,
        row_height: f32,
    ) -> Self {
        // (in-)validate the cache
        cache.set_query(query_expression.into());

        Self {
            row_height,
            cache,
            egui_ctx,
            id,
        }
    }

    /// Implementation for [`egui_table::TableDelegate::row_top_offset`].
    pub(crate) fn row_top_offset(&self, row_nr: u64) -> f32 {
        self.cache
            .expanded_rows
            .range(0..row_nr)
            .map(|(expanded_row_nr, expanded)| {
                self.egui_ctx.animate_value_with_time(
                    self.row_id(*expanded_row_nr),
                    *expanded as f32 * self.row_height,
                    self.egui_ctx.style().animation_time,
                )
            })
            .sum::<f32>()
            + row_nr as f32 * self.row_height
    }

    /// Return by how much row space this row is expended.
    pub(crate) fn row_expansion(&self, row_nr: u64) -> u64 {
        self.cache.expanded_rows.get(&row_nr).copied().unwrap_or(0)
    }

    /// Set the expansion of a row.
    ///
    /// Units are in extra row heights.
    pub(crate) fn set_row_expansion(&mut self, row_nr: u64, additional_row_space: u64) {
        // Note: don't delete the entry when set to 0, this breaks animation.

        // If this is the first time this row is expended, we must seed the corresponding animation
        // cache.
        if !self.cache.expanded_rows.contains_key(&row_nr) {
            self.egui_ctx.animate_value_with_time(
                self.row_id(row_nr),
                0.0,
                self.egui_ctx.style().animation_time,
            );
        }

        self.cache
            .expanded_rows
            .insert(row_nr, additional_row_space);
    }

    /// Collapse a row.
    pub(crate) fn collapse_row(&mut self, row_nr: u64) {
        self.set_row_expansion(row_nr, 0);
    }

    #[inline]
    fn row_id(&self, row_nr: u64) -> egui::Id {
        self.id.with(row_nr)
    }
}
