use std::collections::BTreeMap;

/// Storage for [`ExpandedRows`], which should be persisted across frames.
///
/// Note: each view should store its own cache. Using a [`re_viewer_context::ViewState`] is a
/// good way to do this.
#[derive(Debug, Clone)]
pub(crate) struct ExpandedRowsCache {
    /// Maps "table row number" to "additional lines".
    ///
    /// When expanded, the base space is still used for the summary, while the additional lines are
    /// used for instances.
    expanded_rows: BTreeMap<u64, u64>,

    /// ID used to invalidate the cache.
    valid_for: egui::Id,
}

impl Default for ExpandedRowsCache {
    fn default() -> Self {
        Self {
            expanded_rows: BTreeMap::default(),
            valid_for: egui::Id::new(""),
        }
    }
}

impl ExpandedRowsCache {
    /// This sets the query used for cache invalidation.
    ///
    /// If the query doesn't match the cached one, the state will be reset.
    fn validate_id(&mut self, id: egui::Id) {
        if id != self.valid_for {
            self.valid_for = id;
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
    /// `id` is used to store the animation state and invalidate the cache, make it persistent and
    /// unique
    pub(crate) fn new(
        egui_ctx: egui::Context,
        id: egui::Id,
        cache: &'a mut ExpandedRowsCache,
        row_height: f32,
    ) -> Self {
        // (in-)validate the cache
        cache.validate_id(id);

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
            .map(|(expanded_row_nr, additional_lines)| {
                self.egui_ctx.animate_value_with_time(
                    self.row_id(*expanded_row_nr),
                    *additional_lines as f32 * self.row_height,
                    self.egui_ctx.global_style().animation_time,
                )
            })
            .sum::<f32>()
            + row_nr as f32 * self.row_height
    }

    /// Returns whether the first line of the specified row is odd.
    ///
    /// This depends on how many additional lines the rows before have.
    pub(crate) fn is_row_odd(&self, row_nr: u64) -> bool {
        let total_lines = self
            .cache
            .expanded_rows
            .range(0..row_nr)
            .map(|(_, additional_lines)| *additional_lines)
            .sum::<u64>()
            + row_nr;

        total_lines % 2 == 1
    }

    /// Return by how many additional lines this row is expanded.
    pub(crate) fn additional_lines_for_row(&self, row_nr: u64) -> u64 {
        self.cache.expanded_rows.get(&row_nr).copied().unwrap_or(0)
    }

    /// Set the expansion of a row.
    ///
    /// Units are in extra row heights.
    pub(crate) fn set_additional_lines_for_row(&mut self, row_nr: u64, additional_lines: u64) {
        // Note: don't delete the entry when set to 0, this breaks animation.

        // If this is the first time this row is expanded, we must seed the corresponding animation
        // cache.
        if !self.cache.expanded_rows.contains_key(&row_nr) {
            self.egui_ctx.animate_value_with_time(
                self.row_id(row_nr),
                0.0,
                self.egui_ctx.global_style().animation_time,
            );
        }

        self.cache.expanded_rows.insert(row_nr, additional_lines);
    }

    /// Collapse a row.
    pub(crate) fn remove_additional_lines_for_row(&mut self, row_nr: u64) {
        self.set_additional_lines_for_row(row_nr, 0);
    }

    #[inline]
    fn row_id(&self, row_nr: u64) -> egui::Id {
        self.id.with(row_nr)
    }
}
