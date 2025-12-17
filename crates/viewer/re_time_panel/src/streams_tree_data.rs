use std::ops::{ControlFlow, Range};

use itertools::Itertools as _;
use re_chunk_store::ChunkStore;
use re_data_ui::{ArchetypeComponentMap, sorted_component_list_by_archetype_for_ui};
use re_entity_db::{EntityTree, InstancePath};
use re_log_types::{ComponentPath, EntityPath};
use re_sdk_types::ComponentDescriptor;
use re_ui::filter_widget::{FilterMatcher, PathRanges};
use re_viewer_context::{CollapseScope, Item, ViewerContext, VisitorControlFlow};
use smallvec::SmallVec;

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
    /// is because _currently_, we rarely need to visit, but when we do, we need components, and
    /// having them in the structure would be too expensive for the cases where it's unnecessary
    /// (e.g., when the tree is collapsed).
    ///
    /// The provided closure is called once for each entity with `None` as component argument.
    /// Then, consistent with the display order, its children entities are visited, and then its
    /// components are visited.
    pub fn visit<B>(
        &self,
        viewer_context: &ViewerContext<'_>,
        entity_db: &re_entity_db::EntityDb,
        mut visitor: impl FnMut(EntityOrComponentData<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        let engine = entity_db.storage_engine();
        let store = engine.store();

        for child in &self.children {
            child.visit(viewer_context, store, &mut visitor)?;
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

    pub children: Vec<Self>,
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
        let mut label = entity_part_ui_string
            .clone()
            .unwrap_or_else(|| "/".to_owned());

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

            if !node_info.is_leaf && !entity_tree.path.is_root() {
                // Indicate that we have children
                label.push('/');
            }
            Self {
                entity_path: entity_tree.path.clone(),
                label,
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
        viewer_context: &ViewerContext<'_>,
        store: &ChunkStore,
        visitor: &mut impl FnMut(EntityOrComponentData<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if visitor(EntityOrComponentData::Entity(self)).visit_children()? {
            for child in &self.children {
                child.visit(viewer_context, store, visitor)?;
            }

            for (_, component_descriptors) in
                components_for_entity(viewer_context, store, &self.entity_path)
            {
                for component_descriptor in component_descriptors {
                    // these cannot have children
                    let _ = visitor(EntityOrComponentData::Component {
                        entity_data: self,
                        component_descriptor,
                    })
                    .visit_children()?;
                }
            }
        }

        ControlFlow::Continue(())
    }

    pub fn item(&self) -> Item {
        Item::InstancePath(InstancePath::entity_all(self.entity_path.clone()))
    }

    pub fn is_open(&self, ctx: &egui::Context, collapse_scope: CollapseScope) -> bool {
        collapse_scope
            .item(self.item())
            .is_some_and(|collapse_id| collapse_id.is_open(ctx).unwrap_or(self.default_open))
    }
}

/// Lists the components to be displayed for the given entity
pub fn components_for_entity(
    viewer_context: &ViewerContext<'_>,
    store: &ChunkStore,
    entity_path: &EntityPath,
) -> ArchetypeComponentMap {
    if let Some(components) = store.all_components_for_entity(entity_path) {
        sorted_component_list_by_archetype_for_ui(
            viewer_context.reflection(),
            components
                .iter()
                .filter_map(|component| store.entity_component_descriptor(entity_path, *component)),
        )
    } else {
        ArchetypeComponentMap::default()
    }
}

// ---

#[derive(Debug)]
pub enum EntityOrComponentData<'a> {
    Entity(&'a EntityData),
    Component {
        entity_data: &'a EntityData,
        component_descriptor: ComponentDescriptor,
    },
}

impl EntityOrComponentData<'_> {
    pub fn item(&self) -> Item {
        match self {
            Self::Entity(entity_data) => entity_data.item(),
            Self::Component {
                entity_data,
                component_descriptor,
            } => Item::ComponentPath(ComponentPath::new(
                entity_data.entity_path.clone(),
                component_descriptor.component,
            )),
        }
    }

    pub fn is_open(&self, ctx: &egui::Context, collapse_scope: CollapseScope) -> bool {
        match self {
            Self::Entity(entity_data) => entity_data.is_open(ctx, collapse_scope),
            Self::Component { .. } => true,
        }
    }
}
