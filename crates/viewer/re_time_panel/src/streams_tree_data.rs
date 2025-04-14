use std::ops::{ControlFlow, Range};

use itertools::Itertools as _;
use smallvec::SmallVec;

use re_chunk_store::ChunkStore;
use re_data_ui::sorted_component_list_for_ui;
use re_entity_db::{EntityTree, InstancePath};
use re_log_types::EntityPath;
use re_types_core::ComponentName;
use re_ui::filter_widget::{FilterMatcher, PathRanges};
use re_viewer_context::{CollapseScope, Item, ViewerContext, VisitorControlFlow};

use crate::time_panel::TimePanelSource;

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
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

        let mut hierarchy = Vec::default();
        let mut hierarchy_highlights = PathRanges::default();
        let root_data = EntityData::from_entity_tree_and_filter(
            db.tree(),
            filter_matcher,
            &mut hierarchy,
            &mut hierarchy_highlights,
        );

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

    /// Visit the entire tree.
    ///
    /// Note that we ALSO visit components, despite them not being part of the data structures. This
    /// is because _currently_, we rarely need to visit, but when we do, we need to components, and
    /// having them in the structure would be too expensive for the cases where it's unnecessary
    /// (e.g., when the tree is collapsed).
    ///
    /// The provided closure is called once for each entity with `None` as component name argument.
    /// Then, consistent with the display order, its children entities are visited, and then its
    /// components are visited.
    pub fn visit<B>(
        &self,
        entity_db: &re_entity_db::EntityDb,
        mut visitor: impl FnMut(&EntityData, Option<ComponentName>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        let engine = entity_db.storage_engine();
        let store = engine.store();

        for child in &self.children {
            child.visit(store, &mut visitor)?;
        }

        ControlFlow::Continue(())
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
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
        hierarchy: &mut Vec<String>,
        hierarchy_highlights: &mut PathRanges,
    ) -> Option<Self> {
        let entity_part_ui_string = entity_tree
            .path
            .last()
            .map(|entity_part| entity_part.ui_string());
        let mut label = entity_part_ui_string.clone().unwrap_or("/".to_owned());

        let must_pop = if let Some(part) = &entity_part_ui_string {
            hierarchy.push(part.clone());
            true
        } else {
            false
        };

        //
        // Gather some info about the current node…
        //

        /// Temporary structure to hold local information.
        struct NodeInfo {
            is_leaf: bool,
            is_this_a_match: bool,
            children: Vec<EntityData>,
            default_open: bool,
        }

        let node_info = if entity_tree.children.is_empty() {
            // Key insight: we only ever need to match the hierarchy from the leaf nodes.
            // Non-leaf nodes know they are a match if any child remains after walking their
            // subtree.

            let highlights = filter_matcher.match_path(hierarchy.iter().map(String::as_str));

            let is_this_a_match = if let Some(highlights) = highlights {
                hierarchy_highlights.merge(highlights);
                true
            } else {
                false
            };

            NodeInfo {
                is_leaf: true,
                is_this_a_match,
                children: vec![],
                default_open: false,
            }
        } else {
            let children = entity_tree
                .children
                .values()
                .filter_map(|sub_tree| {
                    Self::from_entity_tree_and_filter(
                        sub_tree,
                        filter_matcher,
                        hierarchy,
                        hierarchy_highlights,
                    )
                })
                .collect_vec();

            let is_this_a_match = !children.is_empty();
            let default_open = filter_matcher.is_active()
                || (entity_tree.path.len() <= 1 && !entity_tree.path.is_reserved());

            NodeInfo {
                is_leaf: false,
                is_this_a_match,
                children,
                default_open,
            }
        };

        //
        // …then handle the node accordingly.
        //

        let result = node_info.is_this_a_match.then(|| {
            let highlight_sections = hierarchy_highlights
                .remove(hierarchy.len().saturating_sub(1))
                .map(Iterator::collect)
                .unwrap_or_default();

            Self {
                entity_path: entity_tree.path.clone(),
                label: if node_info.is_leaf || entity_tree.path.is_root() {
                    label
                } else {
                    // Indicate that we have children
                    label.push('/');
                    label
                },
                highlight_sections,
                default_open: node_info.default_open,
                children: node_info.children,
            }
        });

        if must_pop {
            hierarchy_highlights.remove(hierarchy.len().saturating_sub(1));
            hierarchy.pop();
        }

        result
    }

    /// Visit this entity, included its components in the provided store.
    pub fn visit<B>(
        &self,
        store: &ChunkStore,
        visitor: &mut impl FnMut(&Self, Option<ComponentName>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if visitor(self, None).visit_children()? {
            for child in &self.children {
                child.visit(store, visitor)?;
            }

            for component_name in components_for_entity(store, &self.entity_path) {
                // these cannot have children
                let _ = visitor(self, Some(component_name)).visit_children()?;
            }
        }

        ControlFlow::Continue(())
    }

    pub fn item(&self) -> Item {
        Item::InstancePath(InstancePath::entity_all(self.entity_path.clone()))
    }

    pub fn is_open(&self, ctx: &egui::Context, collapse_scope: CollapseScope) -> Option<bool> {
        collapse_scope
            .item(self.item())
            .map(|collapse_id| collapse_id.is_open(ctx).unwrap_or(self.default_open))
    }
}

/// Lists the components to be displayed for the given entity
pub fn components_for_entity(
    store: &ChunkStore,
    entity_path: &EntityPath,
) -> impl Iterator<Item = ComponentName> {
    if let Some(components) = store.all_components_for_entity(entity_path) {
        itertools::Either::Left(sorted_component_list_for_ui(components.iter()).into_iter())
    } else {
        itertools::Either::Right(std::iter::empty())
    }
}
