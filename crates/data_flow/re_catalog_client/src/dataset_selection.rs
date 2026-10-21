//! Client-side selections over a dataset.

use std::collections::BTreeSet;

use re_log_types::EntryId;
use re_types_core::SegmentId;

/// Which segments of a dataset a [`DatasetSelection`] keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentSelection {
    /// Every segment, including segments registered after the selection was made.
    All,

    /// Only these segments.
    ///
    /// An empty set selects nothing.
    /// IDs that the dataset does not hold are kept, and match nothing.
    Only(BTreeSet<SegmentId>),
}

/// A dataset together with the restrictions applied to it.
///
/// Filters only ever narrow a selection: chaining them intersects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetSelection {
    dataset_id: EntryId,
    segments: SegmentSelection,
}

impl From<EntryId> for DatasetSelection {
    fn from(dataset_id: EntryId) -> Self {
        Self::new(dataset_id)
    }
}

impl DatasetSelection {
    /// A selection of everything in the dataset.
    pub fn new(dataset_id: EntryId) -> Self {
        Self {
            dataset_id,
            segments: SegmentSelection::All,
        }
    }

    pub fn dataset_id(&self) -> EntryId {
        self.dataset_id
    }

    pub fn segments(&self) -> &SegmentSelection {
        &self.segments
    }

    /// The selected segment IDs, or `None` when all segments are selected.
    pub fn segment_filter(&self) -> Option<&BTreeSet<SegmentId>> {
        match &self.segments {
            SegmentSelection::All => None,
            SegmentSelection::Only(ids) => Some(ids),
        }
    }

    /// Whether the selection is known to keep no segment, whatever the dataset holds.
    ///
    /// Protocol requests treat an empty segment-ID list as "every segment",
    /// so callers must check this before sending [`Self::segment_filter`] as one.
    pub fn selects_no_segments(&self) -> bool {
        self.segment_filter().is_some_and(BTreeSet::is_empty)
    }

    /// Restrict the selection to `segment_ids`, intersected with any existing segment restriction.
    #[must_use]
    pub fn filter_segments(&self, segment_ids: impl IntoIterator<Item = SegmentId>) -> Self {
        let new_ids: BTreeSet<SegmentId> = segment_ids.into_iter().collect();
        let segments = match &self.segments {
            SegmentSelection::All => new_ids,
            SegmentSelection::Only(existing) => existing.intersection(&new_ids).cloned().collect(),
        };

        Self {
            dataset_id: self.dataset_id,
            segments: SegmentSelection::Only(segments),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(ids: &[&str]) -> BTreeSet<SegmentId> {
        ids.iter().map(|id| SegmentId::from(*id)).collect()
    }

    fn selection() -> DatasetSelection {
        DatasetSelection::new(EntryId::new())
    }

    #[test]
    fn new_selects_all_segments() {
        let selection = selection();
        assert_eq!(selection.segments(), &SegmentSelection::All);
        assert_eq!(selection.segment_filter(), None);
        assert!(!selection.selects_no_segments());
    }

    #[test]
    fn filter_on_all_keeps_given_ids() {
        let filtered = selection().filter_segments(ids(&["a", "b", "b"]));
        assert_eq!(filtered.segment_filter(), Some(&ids(&["a", "b"])));
    }

    #[test]
    fn chained_filters_intersect() {
        let filtered = selection()
            .filter_segments(ids(&["a", "b", "c"]))
            .filter_segments(ids(&["b", "c", "d"]));
        assert_eq!(filtered.segment_filter(), Some(&ids(&["b", "c"])));
    }

    #[test]
    fn empty_filter_selects_nothing() {
        let filtered = selection().filter_segments([]);
        assert_eq!(
            filtered.segments(),
            &SegmentSelection::Only(BTreeSet::new())
        );
        assert!(filtered.selects_no_segments());
    }

    #[test]
    fn disjoint_filters_select_nothing() {
        let filtered = selection()
            .filter_segments(ids(&["a"]))
            .filter_segments(ids(&["b"]));
        assert_eq!(
            filtered.segments(),
            &SegmentSelection::Only(BTreeSet::new())
        );
        assert!(filtered.selects_no_segments());
    }

    #[test]
    fn filtering_keeps_dataset_id() {
        let base = selection();
        let filtered = base.filter_segments(ids(&["a"]));
        assert_eq!(filtered.dataset_id(), base.dataset_id());
    }
}
