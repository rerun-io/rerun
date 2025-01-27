use std::ops::Range;

use itertools::Itertools as _;
use smallvec::SmallVec;

use re_entity_db::EntityTree;
use re_log_types::EntityPath;
use re_ui::filter_widget::FilterMatcher;
use re_viewer_context::ViewerContext;

use crate::TimePanelSource;

#[derive(Debug)]
pub struct StreamsTreeData {
    pub children: Vec<EntityData>,
}

impl StreamsTreeData {
    pub fn from_source_and_filter(
        ctx: &ViewerContext<'_>,
        source: TimePanelSource,
        filter_matcher: &FilterMatcher,
    ) -> Self {
        re_tracing::profile_function!();

        let db = match source {
            TimePanelSource::Recording => ctx.recording(),
            TimePanelSource::Blueprint => ctx.blueprint_db(),
        };

        let root_data = EntityData::from_entity_tree_and_filter(db.tree(), filter_matcher, false);

        // We show "/" on top only for recording streams, because the `/` entity in blueprint
        // is always empty, so it's just lost space. This works around an issue where the
        // selection/hover state of the `/` entity is wrongly synchronized between both
        // stores, due to `Item::*` not tracking stores for entity paths.

        Self {
            children: match source {
                TimePanelSource::Recording => root_data
                    .map(|entity_part_data| vec![entity_part_data])
                    .unwrap_or_default(),
                TimePanelSource::Blueprint => root_data
                    .map(|entity_part_data| entity_part_data.children)
                    .unwrap_or_default(),
            },
        }
    }
}

// ---

#[derive(Debug)]
pub struct EntityData {
    pub entity_path: EntityPath,

    pub label: String,
    pub highlight_sections: SmallVec<[Range<usize>; 1]>,

    pub default_open: bool,

    pub children: Vec<EntityData>,
}

impl EntityData {
    pub fn from_entity_tree_and_filter(
        entity_tree: &EntityTree,
        filter_matcher: &FilterMatcher,
        mut is_already_a_match: bool,
    ) -> Option<Self> {
        // Early out
        if filter_matcher.matches_nothing() {
            return None;
        }

        let mut label = entity_tree
            .path
            .last()
            .map(|entity_part| entity_part.ui_string())
            .unwrap_or("/".to_owned());

        //
        // Filtering
        //

        let (entity_part_matches, highlight_sections) = if filter_matcher.matches_everything() {
            // fast path (filter is inactive)
            (true, SmallVec::new())
        } else if let Some(entity_part) = entity_tree.path.last() {
            // Nominal case of matching the hierarchy.
            if let Some(match_sections) = filter_matcher.find_matches(&entity_part.ui_string()) {
                (true, match_sections.collect())
            } else {
                (false, SmallVec::new())
            }
        } else {
            // we are the root, it can never match anything
            (false, SmallVec::new())
        };

        // We want to keep entire branches if a single of its node matches. So we must propagate the
        // "matched" state so we can make the right call when we reach leaf nodes.
        is_already_a_match |= entity_part_matches;

        //
        // Recurse
        //

        if entity_tree.children.is_empty() {
            // Discard a leaf item unless it is already a match.
            is_already_a_match.then(|| {
                // Leaf items are always collapsed by default, even when the filter is active.
                let default_open = false;

                Self {
                    entity_path: entity_tree.path.clone(),
                    label,
                    highlight_sections,
                    default_open,
                    children: vec![],
                }
            })
        } else {
            let children = entity_tree
                .children
                .values()
                .filter_map(|sub_tree| {
                    Self::from_entity_tree_and_filter(sub_tree, filter_matcher, is_already_a_match)
                })
                .collect_vec();

            (is_already_a_match || !children.is_empty()).then(|| {
                // Only top-level non-leaf entities are expanded by default, unless the filter is
                // active.
                let default_open = filter_matcher.is_active() || entity_tree.path.len() <= 1;
                Self {
                    entity_path: entity_tree.path.clone(),
                    label: if children.is_empty() || entity_tree.path.is_root() {
                        label
                    } else {
                        // Indicate that we have children
                        label.push('/');
                        label
                    },
                    highlight_sections,
                    default_open,
                    children,
                }
            })
        }
    }
}
