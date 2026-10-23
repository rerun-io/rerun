use std::ops::Range;
use std::sync::Arc;

use re_chunk_store::{AbsoluteTimeRange, RowId};
use re_log_types::TimeInt;

use crate::text_log_index_cache::TextLogQueryKey;
use crate::text_log_span_index::{TextLogSpanAnchor, TextLogSpanIndex};

/// Estimated geometry with exact row identities in resolved spans.
#[derive(re_byte_size::SizeBytes)]
pub struct TextLogNavigation {
    pub index: Arc<TextLogSpanIndex>,

    /// Identity within which span-relative anchors remain valid.
    pub layout: RowId,

    /// Configuration and cursor time this navigation was planned for.
    pub cursor: Option<(TextLogQueryKey, TimeInt)>,

    /// The reported viewport, including overscan, needs no further navigation refinement.
    /// Display payloads may still need refetching; the UI must confirm viewport stability.
    pub settled: bool,
}

impl Default for TextLogNavigation {
    fn default() -> Self {
        Self {
            index: Arc::default(),
            layout: RowId::ZERO,
            cursor: None,
            settled: false,
        }
    }
}

/// The table's visible row range from the previous frame.
///
/// Positions refer to the filtered table layout, not stable row identities.
#[derive(Clone, PartialEq, Eq, re_byte_size::SizeBytes)]
struct VisibleRows {
    index_key: TextLogQueryKey,
    range: Range<usize>,

    /// Layout identity validating `span_anchor`.
    layout: RowId,

    /// Span-relative anchor for the first visible row.
    /// Preserves the scroll position when refining earlier spans changes global row offsets.
    span_anchor: Option<TextLogSpanAnchor>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, re_byte_size::SizeBytes)]
enum NavigationMode {
    #[default]
    Free,

    /// Center on the cursor unconditionally.
    Recenter,

    /// Center on the cursor once its span is resolved, until the viewport settles there.
    Seek,
}

/// Per-view policy; shared indexes contain no viewport or cursor state.
/// Playhead/configuration changes seek until settled; data arrivals do not restart seeking.
#[derive(Clone, PartialEq, Eq, Default, re_byte_size::SizeBytes)]
pub struct TextLogNavigationController {
    latest: Option<(TextLogQueryKey, TimeInt)>,
    mode: NavigationMode,
    visible_rows: Option<VisibleRows>,
}

pub enum TextLogScroll {
    Center(usize),
    Top(usize),
}

/// Viewport planned from the previous frame's feedback, so queries can run without mutating the view.
pub struct TextLogNavigationPlan {
    /// Index configuration this plan targets, paired with `cursor_time` when adopting navigation.
    key: TextLogQueryKey,

    /// Layout identity within which the previous viewport's span anchor remains valid.
    layout: RowId,

    /// Playhead time used to position the query window while seeking.
    cursor_time: TimeInt,

    /// Whether to center queries on the playhead rather than preserve the previous viewport.
    seek_to_cursor: bool,

    /// Previous frame's viewport for this index configuration, used for anchoring and settlement.
    /// Its span anchor is only usable when its layout matches `layout`.
    visible: Option<VisibleRows>,

    /// Query-window size inferred from the previous viewport, at least one row.
    /// Defaults to 100 rows before viewport feedback is available.
    row_count: usize,

    /// Span index and time range to resolve next, prioritizing the viewport over overscan.
    /// `None` means neither region needs further refinement.
    pub next_span: Option<(usize, AbsoluteTimeRange)>,

    /// Planned row range including overscan, whose resolved spans must be protected from eviction.
    /// Uses pre-refinement row offsets; convert to span identities before resolving more rows.
    pub protected_rows: Range<usize>,
}

impl TextLogNavigationController {
    fn cursor_changed(&self, key: TextLogQueryKey, time: TimeInt) -> bool {
        self.latest != Some((key, time))
    }

    /// Explicit cursor/configuration changes retain precedence until the viewport settles.
    fn seeks_cursor(&self, key: TextLogQueryKey, time: TimeInt) -> bool {
        self.cursor_changed(key, time) || self.mode != NavigationMode::Free
    }

    pub fn plan(
        &self,
        key: TextLogQueryKey,
        layout: RowId,
        cursor_time: TimeInt,
        index: &TextLogSpanIndex,
    ) -> TextLogNavigationPlan {
        let visible = self
            .visible_rows
            .clone()
            .filter(|visible| visible.index_key == key);
        let mut plan = TextLogNavigationPlan {
            key,
            layout,
            cursor_time,
            seek_to_cursor: self.seeks_cursor(key, cursor_time),
            row_count: visible
                .as_ref()
                .map_or(100, |visible| visible.range.len().max(1)),
            visible,
            next_span: None,
            protected_rows: 0..0,
        };
        let rows = plan.rows(index);
        plan.protected_rows = with_overscan(rows.clone());
        plan.next_span = index
            .next_query(rows.start, rows.end)
            .or_else(|| index.next_query(plan.protected_rows.start, plan.protected_rows.end));
        plan
    }

    /// Adopts the cursor the navigation was planned for, starting a seek if it changed.
    pub fn observe(&mut self, navigation: &TextLogNavigation) {
        let Some((key, time)) = navigation.cursor else {
            return;
        };
        if self.cursor_changed(key, time) {
            self.mode = NavigationMode::Recenter;
        }
        self.latest = Some((key, time));
    }

    pub fn scroll(
        &mut self,
        navigation: &TextLogNavigation,
        table_is_new: bool,
    ) -> Option<TextLogScroll> {
        let (key, time) = self.latest?;
        if table_is_new {
            self.mode = NavigationMode::Recenter;
        }
        if self.mode == NavigationMode::Recenter
            || (self.mode == NavigationMode::Seek && navigation.index.time_is_resolved(time))
        {
            return Some(TextLogScroll::Center(
                navigation.index.row_for_time(time, false),
            ));
        }
        let visible = self.visible_rows.as_ref()?;
        if visible.index_key != key || visible.layout != navigation.layout {
            return None;
        }
        let row = navigation.index.anchor_row(visible.span_anchor?);
        (row != visible.range.start).then_some(TextLogScroll::Top(row))
    }

    /// Settlement uses the actual viewport, not the planned seek-centered query window.
    /// Returns whether the visible rows changed.
    pub fn rendered(
        &mut self,
        navigation: &TextLogNavigation,
        range: Option<Range<usize>>,
    ) -> bool {
        let visible_rows =
            Option::zip(self.latest, range).map(|((index_key, _), range)| VisibleRows {
                index_key,
                layout: navigation.layout,
                span_anchor: navigation.index.anchor(range.start),
                range,
            });

        if let (Some((_, time)), NavigationMode::Recenter | NavigationMode::Seek) =
            (self.latest, self.mode)
        {
            let index = &navigation.index;
            let stable = visible_rows.as_ref().map(|v| &v.range)
                == self.visible_rows.as_ref().map(|v| &v.range);
            let target = index
                .row_for_time(time, false)
                .min(index.len().saturating_sub(1));
            let settled = index.time_is_resolved(time)
                && navigation.settled
                && (index.len() == 0
                    || (stable
                        && visible_rows
                            .as_ref()
                            .is_some_and(|v| v.range.contains(&target))));
            self.mode = if settled {
                NavigationMode::Free
            } else {
                NavigationMode::Seek
            };
        }
        let changed = self.visible_rows != visible_rows;
        self.visible_rows = visible_rows;

        changed
    }
}

impl TextLogNavigationPlan {
    /// Recompute after refinement: both cursor positions and anchor offsets may have moved.
    fn rows(&self, index: &TextLogSpanIndex) -> Range<usize> {
        let start = if self.seek_to_cursor {
            index
                .row_for_time(self.cursor_time, false)
                .min(index.len().saturating_sub(1))
                .saturating_sub(self.row_count / 2)
                .min(index.len().saturating_sub(self.row_count))
        } else if let Some(anchor) = self
            .visible
            .as_ref()
            .filter(|visible| visible.layout == self.layout)
            .and_then(|visible| visible.span_anchor)
        {
            index.anchor_row(anchor)
        } else {
            self.visible
                .as_ref()
                .map_or(0, |visible| visible.range.start)
        };
        start..start.saturating_add(self.row_count)
    }

    pub fn display_rows(&self, index: &TextLogSpanIndex) -> Range<usize> {
        with_overscan(self.rows(index))
    }

    pub fn finish(self, index: Arc<TextLogSpanIndex>) -> TextLogNavigation {
        // Readiness describes the UI's reported viewport, not the seek-centered query window.
        let settled = index.len() == 0
            || self.visible.as_ref().is_some_and(|visible| {
                let rows = with_overscan(visible.range.clone());
                index.next_query(rows.start, rows.end).is_none()
            });
        TextLogNavigation {
            index,
            layout: self.layout,
            cursor: Some((self.key, self.cursor_time)),
            settled,
        }
    }
}

/// We query this many extra rows beyond the visible range to allow for smooth scrolling without immediate re-querying.
///
/// Text logs are usually cheap and often in their own chunks, so we can be rather generous here.
const ROW_OVERSCAN: usize = 1024;

fn with_overscan(rows: Range<usize>) -> Range<usize> {
    rows.start.saturating_sub(ROW_OVERSCAN)..rows.end.saturating_add(ROW_OVERSCAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> TextLogQueryKey {
        TextLogQueryKey::for_test(())
    }

    fn free_controller(key: TextLogQueryKey, time: TimeInt) -> TextLogNavigationController {
        let mut controller = TextLogNavigationController::default();
        let index = Arc::new(TextLogSpanIndex::default());
        let navigation = controller
            .plan(key, RowId::new(), time, &index)
            .finish(index);
        controller.observe(&navigation);
        controller.rendered(&navigation, Some(0..0));
        controller
    }

    /// Refining an earlier span must preserve the visible row identity without restarting a cursor seek.
    #[test]
    fn preceding_refinement_repairs_the_manual_scroll_anchor() {
        use crate::text_log_index_cache::{IndexedRow, TextLogIndex};

        let key = key();
        let time = TimeInt::new_temporal(25);
        let mut controller = free_controller(key, time);
        let span = crate::text_log_span_index::TARGET_SPAN_ROWS;
        let mut index = TextLogSpanIndex::new([
            (AbsoluteTimeRange::new(0, 9), span as u64),
            (AbsoluteTimeRange::new(20, 29), span as u64),
        ]);
        let layout = RowId::new();
        let navigation = |index| TextLogNavigation {
            index: Arc::new(index),
            layout,
            cursor: Some((key, time)),
            settled: false,
        };
        controller.rendered(&navigation(index.clone()), Some(span..span + 5));
        // The first span shrinks from its estimate to 20 rows, shifting the anchored second span.
        let rows = (0..20)
            .map(|instance| IndexedRow {
                time: TimeInt::new_temporal(i64::try_from(instance / 2).unwrap()),
                source: 0,
                row_id: RowId::new(),
                instance: instance % 2,
            })
            .collect();
        let protected = index.protected_range(span..span + 5);
        index.resolve_protected(0, Arc::new(TextLogIndex { rows }), &[protected]);
        let navigation = navigation(index);

        assert!(matches!(
            controller.scroll(&navigation, false),
            Some(TextLogScroll::Top(20))
        ));
        controller.rendered(&navigation, Some(20..25));
        assert!(controller.scroll(&navigation, false).is_none());
        assert!(!controller.seeks_cursor(key, time));
    }

    /// Empty viewports can settle, but a recreated table still needs an explicit scroll target.
    #[test]
    fn empty_seek_settles_and_table_recreation_seeks_again() {
        let mut controller = TextLogNavigationController::default();
        let index = Arc::new(TextLogSpanIndex::default());
        let key = key();
        let time = TimeInt::new_temporal(100);
        let navigation = controller
            .plan(key, RowId::new(), time, &index)
            .finish(index);
        controller.observe(&navigation);
        assert!(matches!(
            controller.scroll(&navigation, false),
            Some(TextLogScroll::Center(0))
        ));
        controller.rendered(&navigation, Some(0..0));
        assert!(!controller.seeks_cursor(key, time));
        assert!(matches!(
            controller.scroll(&navigation, true),
            Some(TextLogScroll::Center(0))
        ));
        assert!(controller.seeks_cursor(key, time));
    }
}
