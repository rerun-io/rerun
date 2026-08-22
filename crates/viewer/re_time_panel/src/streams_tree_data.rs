use std::ops::{ControlFlow, Range};

use itertools::Itertools as _;
use re_chunk::TimelineName;
use re_chunk_store::ChunkStore;
use re_data_ui::{ArchetypeComponentMap, sorted_component_list_by_archetype_for_ui};
use re_entity_db::{EntityTree, InstancePath};
use re_log_types::{ComponentPath, EntityPath, TimeInt};
use re_sdk_types::ComponentDescriptor;
use re_ui::filter_widget::{FilterMatcher, PathRanges};
use re_viewer_context::{AppContext, CollapseScope, Item, ViewerContext, VisitorControlFlow};
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
        // `Some((timeline, cursor, bin_width))` to sort by "temporal relevance" (closeness to
        // `cursor`, in either direction, after rounding event times to the nearest multiple of
        // `bin_width`) instead of the default hierarchical order.
        temporal_relevance: Option<(TimelineName, TimeInt, i64)>,
    ) -> Self {
        re_tracing::profile_function!();

        let db = match source {
            TimePanelSource::Recording => ctx.recording(),
            TimePanelSource::Blueprint => ctx.blueprint_db(),
        };

        let mut hierarchy = Vec::default();
        let mut hierarchy_highlights = PathRanges::default();
        let db_engine = db.storage_engine();
        let root_data = EntityData::from_entity_tree_and_filter(
            db_engine.store().entity_tree(),
            filter_matcher,
            &mut hierarchy,
            &mut hierarchy_highlights,
        );

        // We show "/" on top only for recording streams, because the `/` entity in blueprint
        // is always empty, so it's just lost space. This works around an issue where the
        // selection/hover state of the `/` entity is wrongly synchronized between both
        // stores, due to `Item::*` not tracking stores for entity paths.

        let mut this = Self {
            children: match source {
                TimePanelSource::Recording => root_data
                    .map(|entity_part_data| vec![entity_part_data])
                    .unwrap_or_default(),
                TimePanelSource::Blueprint => root_data
                    .map(|entity_part_data| entity_part_data.children)
                    .unwrap_or_default(),
            },
        };

        if let Some((timeline, cursor, bin_width)) = temporal_relevance {
            sort_children_by_temporal_relevance(
                &mut this.children,
                db_engine.store(),
                &timeline,
                cursor,
                bin_width,
            );
        }

        this
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
        ctx: &AppContext<'_>,
        entity_db: &re_entity_db::EntityDb,
        mut visitor: impl FnMut(EntityOrComponentData<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        let engine = entity_db.storage_engine();
        let store = engine.store();

        for child in &self.children {
            child.visit(ctx, store, &mut visitor)?;
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
        ctx: &AppContext<'_>,
        store: &ChunkStore,
        visitor: &mut impl FnMut(EntityOrComponentData<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if visitor(EntityOrComponentData::Entity(self)).visit_children()? {
            for child in &self.children {
                child.visit(ctx, store, visitor)?;
            }

            for (_, component_descriptors) in components_for_entity(ctx, store, &self.entity_path) {
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

    pub fn is_open(&self, egui_ctx: &egui::Context, collapse_scope: CollapseScope) -> bool {
        collapse_scope
            .item(self.item())
            .is_some_and(|collapse_id| collapse_id.is_open(egui_ctx).unwrap_or(self.default_open))
    }
}

/// Sorts `children` in place by "temporal relevance" -- closeness to `cursor`, in either
/// direction -- recursively, depth-first, and returns the best (smallest) aggregate distance
/// among them, considering both each child's own data and, recursively, all of its descendants'.
///
/// We deliberately sort by *distance* rather than only "next upcoming" or only "most recent
/// past": a purely directional key saw an entity's rank sawtooth across its *entire* update
/// interval every time the cursor passed one of its events (e.g. an entity that updates every
/// 5s would drift up to 5s "away" right after each event, only to snap close again just before
/// the next one). Distance-to-cursor halves that swing, since the moment you pass an event's
/// midpoint to the next one, the nearer neighbor flips to the one just passed -- same idea,
/// meaningfully less shuffling as the cursor moves. It also means we don't need to track which
/// way the user last scrubbed at all.
///
/// Entities (and subtrees) with no temporal data on this timeline always sort last, after every
/// entity that has some.
///
/// `bin_width` (a value of 0 disables it) rounds both `cursor` and every candidate event time to
/// the nearest multiple of `bin_width` before computing distances, so that near-simultaneous
/// events end up tied instead of causing constant reordering over trivial differences (repeated
/// updates a few ms apart on a live-streaming entity, several components on the same entity
/// logged in separate calls, etc).
fn sort_children_by_temporal_relevance(
    children: &mut Vec<EntityData>,
    store: &ChunkStore,
    timeline: &TimelineName,
    cursor: TimeInt,
    bin_width: i64,
) -> Option<u64> {
    let binned_cursor = round_to_bin(cursor.as_i64(), bin_width);

    let mut keyed: Vec<(Option<u64>, EntityData)> = std::mem::take(children)
        .into_iter()
        .map(|mut child| {
            let best_descendant = sort_children_by_temporal_relevance(
                &mut child.children,
                store,
                timeline,
                cursor,
                bin_width,
            );
            let after = store.entity_time_at_or_after(timeline, &child.entity_path, cursor);
            let before = store.entity_time_at_or_before(timeline, &child.entity_path, cursor);
            let own_distance = [after, before]
                .into_iter()
                .flatten()
                .map(|time| round_to_bin(time.as_i64(), bin_width).abs_diff(binned_cursor))
                .min();
            let key = [own_distance, best_descendant].into_iter().flatten().min();

            (key, child)
        })
        .collect();

    keyed.sort_by_key(|(key, _)| key.unwrap_or(u64::MAX));

    let best = keyed.first().and_then(|(key, _)| *key);
    *children = keyed.into_iter().map(|(_, child)| child).collect();
    best
}

/// Rounds `value` to the nearest multiple of `bin_width` (a `bin_width` of 0 or less is a no-op).
///
/// Uses integer arithmetic throughout (never converts through `f64`) since these are nanosecond
/// timestamps that can exceed `f64`'s 53-bit exact-integer range.
fn round_to_bin(value: i64, bin_width: i64) -> i64 {
    if bin_width <= 0 {
        return value;
    }
    // Saturating throughout: `value` can be the `TimeInt::MIN` sentinel (used before any time
    // cursor is set), and rounding that can otherwise overshoot `i64::MIN` on the final multiply.
    value
        .saturating_add(bin_width / 2)
        .div_euclid(bin_width)
        .saturating_mul(bin_width)
}

/// Lists the components to be displayed for the given entity
pub fn components_for_entity(
    ctx: &AppContext<'_>,
    store: &ChunkStore,
    entity_path: &EntityPath,
) -> ArchetypeComponentMap {
    if let Some(components) = store.schema().all_components_for_entity(entity_path) {
        sorted_component_list_by_archetype_for_ui(
            ctx.reflection,
            components.iter().filter_map(|component| {
                store
                    .schema()
                    .entity_component_descriptor(entity_path, *component)
            }),
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

    pub fn is_open(&self, egui_ctx: &egui::Context, collapse_scope: CollapseScope) -> bool {
        match self {
            Self::Entity(entity_data) => entity_data.is_open(egui_ctx, collapse_scope),
            Self::Component { .. } => true,
        }
    }
}
