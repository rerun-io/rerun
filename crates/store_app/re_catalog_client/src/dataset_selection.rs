//! Client-side selections over a dataset.

use std::collections::BTreeSet;

use re_log_types::{EntityPath, EntityPathFilter, EntryId, ResolvedEntityPathFilter};
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

    /// An entity is selected only if every filter matches it; no filter selects every entity.
    //TODO(RR-3157): add the ability to select individual columns.
    content_filters: Vec<ResolvedEntityPathFilter>,
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
            content_filters: Vec::new(),
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
            content_filters: self.content_filters.clone(),
        }
    }

    pub fn content_filters(&self) -> &[ResolvedEntityPathFilter] {
        &self.content_filters
    }

    /// Restrict the selection to the entities `filter` matches, intersected with any existing
    /// content restriction.
    ///
    /// Each filter is resolved on its own, so it excludes properties unless it names them:
    /// a later call can't bring back what an earlier one excluded.
    #[must_use]
    pub fn filter_contents(&self, filter: EntityPathFilter) -> Self {
        let mut selection = self.clone();
        selection
            .content_filters
            .push(filter.resolve_without_substitutions());
        selection
    }

    /// Without a content filter, every entity is selected, properties included.
    pub fn selects_entity(&self, entity_path: &EntityPath) -> bool {
        self.content_filters
            .iter()
            .all(|filter| filter.matches(entity_path))
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

    fn filter(expr: &str) -> EntityPathFilter {
        EntityPathFilter::parse_forgiving(expr)
    }

    fn selects(selection: &DatasetSelection, entity_path: &str) -> bool {
        selection.selects_entity(&EntityPath::from(entity_path))
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

    #[test]
    fn new_selects_every_entity_including_properties() {
        let selection = selection();
        assert!(selection.content_filters().is_empty());
        assert!(selects(&selection, "/anything/at/all"));
        assert!(selects(&selection, "/__properties"));
    }

    #[test]
    fn empty_content_filter_selects_nothing() {
        let filtered = selection().filter_contents(filter(""));
        assert!(!selects(&filtered, "/points"));
        assert!(!selects(&filtered, "/__properties"));
    }

    #[test]
    fn content_filter_excludes_properties_unless_named() {
        assert!(!selects(
            &selection().filter_contents(filter("/**")),
            "/__properties"
        ));
        assert!(selects(
            &selection().filter_contents(filter("/** /__properties/**")),
            "/__properties"
        ));
    }

    #[test]
    fn chained_content_filters_intersect() {
        let filtered = selection()
            .filter_contents(filter("/robot/**"))
            .filter_contents(filter("/robot/arm/**"));
        assert!(selects(&filtered, "/robot/arm/elbow"));
        assert!(!selects(&filtered, "/robot/base"));
    }

    #[test]
    fn later_content_filter_cannot_include_what_an_earlier_one_excluded() {
        let filtered = selection()
            .filter_contents(filter("/robot/** -/robot/arm/**"))
            .filter_contents(filter("/robot/arm/gripper"));
        assert!(!selects(&filtered, "/robot/arm/gripper"));
        assert!(!selects(&filtered, "/robot/base"));
    }

    #[test]
    fn later_content_filter_can_exclude_from_everything() {
        let filtered = selection()
            .filter_contents(filter("/robot/**"))
            .filter_contents(filter("/** -/robot/debug/**"));
        assert!(selects(&filtered, "/robot/arm"));
        assert!(!selects(&filtered, "/robot/debug/trace"));
        assert!(!selects(&filtered, "/camera"));
    }

    #[test]
    fn exclusion_only_content_filter_selects_nothing() {
        let filtered = selection()
            .filter_contents(filter("/robot/**"))
            .filter_contents(filter("-/robot/debug/**"));
        assert!(!selects(&filtered, "/robot/arm"));
        assert!(!selects(&filtered, "/robot/debug/trace"));
    }

    #[test]
    fn properties_named_in_one_content_filter_only_stay_excluded() {
        let filtered = selection()
            .filter_contents(filter("/** /__properties/**"))
            .filter_contents(filter("/**"));
        assert!(selects(&filtered, "/points"));
        assert!(!selects(&filtered, "/__properties"));
    }

    #[test]
    fn segment_and_content_filters_are_independent() {
        let filtered = selection()
            .filter_segments(ids(&["a", "b"]))
            .filter_contents(filter("/points/**"))
            .filter_segments(ids(&["b"]));
        assert_eq!(filtered.segment_filter(), Some(&ids(&["b"])));
        assert!(selects(&filtered, "/points/a"));
        assert!(!selects(&filtered, "/text"));
    }
}
