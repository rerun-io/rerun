//! Data structure describing the contents of the blueprint tree
//!
//! The design goal of these structures is to cover the entire underlying tree (container hierarchy
//! and data result hierarchies) by walking it once entirely, applying filtering on the way.
//!
//! This is done regardless of whether the data is actually used by the UI (e.g. if everything is
//! collapsed). Benchmarks have indicated that this approach incurs a negligible overhead compared
//! to the overall cost of having large blueprint trees (a.k.a the many-entities performance
//! issues: <https://github.com/rerun-io/rerun/issues/8233>).

use std::ops::{ControlFlow, Range};

use itertools::Itertools as _;
use re_entity_db::InstancePath;
use re_log::debug_assert;
use re_log_types::EntityPath;
use re_log_types::external::re_types_core::ViewClassIdentifier;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_ui::filter_widget::{FilterMatcher, PathRanges};
use re_viewer_context::{
    CollapseScope, ContainerId, Contents, ContentsName, DataQueryResult,
    DataResultInteractionAddress, DataResultNode, Item, ViewId, ViewerContext, VisitorControlFlow,
};
use re_viewport_blueprint::{ContainerBlueprint, ViewBlueprint, ViewportBlueprint};
use smallvec::SmallVec;

use crate::data_result_node_or_path::DataResultNodeOrPath;

/// Top-level blueprint tree structure.
#[derive(Debug, Default)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct BlueprintTreeData {
    pub root_container: Option<ContainerData>,
}

impl BlueprintTreeData {
    pub fn from_blueprint_and_filter(
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        filter_matcher: &FilterMatcher,
    ) -> Self {
        re_tracing::profile_function!();

        Self {
            root_container: viewport_blueprint
                .container(&viewport_blueprint.root_container)
                .and_then(|container_blueprint| {
                    ContainerData::from_blueprint_and_filter(
                        ctx,
                        viewport_blueprint,
                        container_blueprint,
                        filter_matcher,
                    )
                }),
        }
    }

    pub fn visit<B>(
        &self,
        mut visitor: impl FnMut(BlueprintTreeItem<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if let Some(root_container) = &self.root_container {
            root_container.visit(&mut visitor)
        } else {
            ControlFlow::Continue(())
        }
    }
}

// ---

/// Data for either a container or a view (both of which possible child of a container).
#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub enum ContentsData {
    Container(ContainerData),
    View(ViewData),
}

impl ContentsData {
    pub fn visit<B>(
        &self,
        visitor: &mut impl FnMut(BlueprintTreeItem<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        match &self {
            Self::Container(container_data) => container_data.visit(visitor),
            Self::View(view_data) => view_data.visit(visitor),
        }
    }
}

/// Data related to a single container and its children.
#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct ContainerData {
    #[cfg_attr(feature = "testing", serde(skip))]
    pub id: ContainerId,
    pub name: ContentsName,
    pub kind: egui_tiles::ContainerKind,
    pub visible: bool,
    pub default_open: bool,

    pub children: Vec<ContentsData>,
}

impl ContainerData {
    pub fn from_blueprint_and_filter(
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        container_blueprint: &ContainerBlueprint,
        filter_matcher: &FilterMatcher,
    ) -> Option<Self> {
        let children = container_blueprint
            .contents
            .iter()
            .filter_map(|content| match content {
                Contents::Container(container_id) => {
                    if let Some(container_blueprint) = viewport_blueprint.container(container_id) {
                        Self::from_blueprint_and_filter(
                            ctx,
                            viewport_blueprint,
                            container_blueprint,
                            filter_matcher,
                        )
                        .map(ContentsData::Container)
                    } else {
                        re_log::warn_once!(
                            "Failed to find container {container_id} in ViewportBlueprint"
                        );
                        None
                    }
                }
                Contents::View(view_id) => {
                    if let Some(view_blueprint) = viewport_blueprint.view(view_id) {
                        ViewData::from_blueprint_and_filter(ctx, view_blueprint, filter_matcher)
                            .map(ContentsData::View)
                    } else {
                        re_log::warn_once!("Failed to find view {view_id} in ViewportBlueprint");
                        None
                    }
                }
            })
            .collect_vec();

        // everything was filtered out
        if filter_matcher.is_active() && children.is_empty() {
            return None;
        }

        Some(Self {
            id: container_blueprint.id,
            name: container_blueprint.display_name_or_default(),
            kind: container_blueprint.container_kind,
            visible: container_blueprint.visible,
            default_open: true,
            children,
        })
    }

    pub fn visit<B>(
        &self,
        visitor: &mut impl FnMut(BlueprintTreeItem<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if visitor(BlueprintTreeItem::Container(self)).visit_children()? {
            for child in &self.children {
                child.visit(visitor)?;
            }
        }

        ControlFlow::Continue(())
    }

    pub fn item(&self) -> Item {
        Item::Container(self.id)
    }
}

// ---

/// Data related to a single view and its content.
#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct ViewData {
    #[cfg_attr(feature = "testing", serde(skip))]
    pub id: ViewId,

    pub class_identifier: ViewClassIdentifier,

    pub name: ContentsName,
    pub visible: bool,
    pub default_open: bool,

    /// The origin tree contains the data results contained in the subtree defined by the view
    /// origin. They are presented first in the blueprint tree.
    pub origin_tree: Option<DataResultData>,

    /// Projection trees are the various trees contained data results which are outside the view
    /// origin subtrees. They are presented after a "Projections:" label in the blueprint tree.
    ///
    /// These trees may be super-trees of the view origin trees. In that case, the view origin
    /// subtree is represented by a stub item, see [`DataResultKind::OriginProjectionPlaceholder`].
    pub projection_trees: Vec<DataResultData>,
}

impl ViewData {
    fn from_blueprint_and_filter(
        ctx: &ViewerContext<'_>,
        view_blueprint: &ViewBlueprint,
        filter_matcher: &FilterMatcher,
    ) -> Option<Self> {
        re_tracing::profile_function!();

        let query_result = ctx.lookup_query_result(view_blueprint.id);
        let result_tree = &query_result.tree;

        //
        // Data results within the view origin subtree
        //

        let mut hierarchy = Vec::with_capacity(10);
        let mut hierarchy_highlights = PathRanges::default();
        let origin_tree = DataResultData::from_data_result_and_filter(
            view_blueprint,
            query_result,
            &DataResultNodeOrPath::from_path_lookup(result_tree, &view_blueprint.space_origin),
            false,
            &mut hierarchy,
            &mut hierarchy_highlights,
            filter_matcher,
        );

        debug_assert!(hierarchy.is_empty());
        hierarchy.clear();
        hierarchy_highlights.clear();

        //
        // Data results outside the view origin subtree (a.k.a projections)
        //

        let mut projections = Vec::new();
        result_tree.visit(&mut |node| {
            if node
                .data_result
                .entity_path
                .starts_with(&view_blueprint.space_origin)
            {
                // If it's under the origin, we're not interested, stop recursing.
                false
            } else if node.data_result.tree_prefix_only {
                // Keep recursing until we find a projection.
                true
            } else {
                projections.push(node);

                // No further recursion needed in this branch, everything below is included
                // in the projection (or shouldn't be included if the projection has already
                // been filtered out).
                false
            }
        });

        let projection_trees = projections
            .into_iter()
            .filter_map(|node| {
                re_tracing::profile_scope!("from_data_result_and_filter");
                let projection_tree = DataResultData::from_data_result_and_filter(
                    view_blueprint,
                    query_result,
                    &DataResultNodeOrPath::DataResultNode(node),
                    true,
                    &mut hierarchy,
                    &mut hierarchy_highlights,
                    filter_matcher,
                );

                debug_assert!(hierarchy.is_empty());
                hierarchy.clear();
                hierarchy_highlights.clear();

                projection_tree
            })
            .collect_vec();

        if origin_tree.is_none() && projection_trees.is_empty() {
            return None;
        }

        let default_open = filter_matcher.is_active()
            || origin_tree.as_ref().is_none_or(|data_result_data| {
                default_open_for_data_result(data_result_data.children.len())
            });

        Some(Self {
            id: view_blueprint.id,
            class_identifier: view_blueprint.class_identifier(),
            name: view_blueprint.display_name_or_default(),
            visible: view_blueprint.visible,
            default_open,
            origin_tree,
            projection_trees,
        })
    }

    pub fn visit<B>(
        &self,
        visitor: &mut impl FnMut(BlueprintTreeItem<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if visitor(BlueprintTreeItem::View(self)).visit_children()? {
            if let Some(origin_tree) = &self.origin_tree {
                origin_tree.visit(visitor)?;
            }

            for projection_tree in &self.projection_trees {
                projection_tree.visit(visitor)?;
            }
        }

        ControlFlow::Continue(())
    }

    pub fn item(&self) -> Item {
        Item::View(self.id)
    }
}

// ---

/// The various kind of things that may be represented in a data result tree.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub enum DataResultKind {
    /// This is a regular entity part of a data result (or the tree that contains it).
    EntityPart,

    /// When the view has no data result contained in the view origin subtree, we still display the
    /// origin with a warning styling to highlight what is likely an undesired view configuration.
    EmptyOriginPlaceholder,

    /// Since the view origin tree is displayed on its own, we don't repeat it within projections
    /// and instead replace it with this placeholder.
    OriginProjectionPlaceholder,
}

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct DataResultData {
    pub kind: DataResultKind,
    pub entity_path: EntityPath,
    pub visible: bool,

    // Exclude the id from serialization for snapshot tests. We could also do the redaction later, but `VisualizerInstructionId` doesn't implement `serde::Serialize` to begin with.
    #[cfg_attr(feature = "testing", serde(skip))]
    pub visualizer_instruction_ids: Vec<VisualizerInstructionId>,

    #[cfg_attr(feature = "testing", serde(skip))]
    pub view_id: ViewId,

    /// Label that should be used for display.
    ///
    /// This typically corresponds to `entity_path.last().ui_string()` but covers corner cases.
    pub label: String,

    /// The sections within the label that correspond to a filter match and should thus be
    /// highlighted.
    pub highlight_sections: SmallVec<[Range<usize>; 1]>,

    pub default_open: bool,
    pub children: Vec<Self>,
}

impl DataResultData {
    fn from_data_result_and_filter(
        view_blueprint: &ViewBlueprint,
        query_result: &DataQueryResult,
        data_result_or_path: &DataResultNodeOrPath<'_>,
        projection: bool,
        hierarchy: &mut Vec<String>,
        hierarchy_highlights: &mut PathRanges,
        filter_matcher: &FilterMatcher,
    ) -> Option<Self> {
        // No profile scope on this recursive function

        let entity_path = data_result_or_path.path().clone();
        let data_result_node = data_result_or_path.data_result_node();
        let visible = data_result_node.is_some_and(|node| node.data_result.is_visible());

        let entity_part_ui_string = entity_path
            .last()
            .map(|entity_part| entity_part.ui_string());

        let (label, should_pop) = if let Some(entity_part_ui_string) = entity_part_ui_string.clone()
        {
            hierarchy.push(entity_part_ui_string.clone());
            (entity_part_ui_string, true)
        } else {
            ("/ (root)".to_owned(), false)
        };

        //
        // Gather some info about the current node…
        //

        enum LeafOrNot<'a> {
            Leaf,
            NotLeaf(&'a DataResultNode),
        }

        /// Temporary structure to hold local information.
        struct NodeInfo<'a> {
            leaf_or_not: LeafOrNot<'a>,
            kind: DataResultKind,
            default_open: bool,
        }

        #[expect(clippy::manual_map)]
        let node_info = if projection {
            if entity_path == view_blueprint.space_origin {
                Some(NodeInfo {
                    leaf_or_not: LeafOrNot::Leaf,
                    kind: DataResultKind::OriginProjectionPlaceholder,
                    // not hierarchical anyway
                    default_open: false,
                })
            } else if let Some(data_result_node) = data_result_node {
                Some(NodeInfo {
                    leaf_or_not: if data_result_node.children.is_empty() {
                        LeafOrNot::Leaf
                    } else {
                        LeafOrNot::NotLeaf(data_result_node)
                    },
                    kind: DataResultKind::EntityPart,
                    // projections are collapsed by default
                    default_open: filter_matcher.is_active(),
                })
            } else {
                // TODO(ab): what are the circumstances for this? Should we warn about it?
                None
            }
        } else {
            // empty origin case
            if entity_path == view_blueprint.space_origin && data_result_node.is_none() {
                Some(NodeInfo {
                    leaf_or_not: LeafOrNot::Leaf,
                    kind: DataResultKind::EmptyOriginPlaceholder,
                    // not hierarchical anyway
                    default_open: false,
                })
            } else if let Some(data_result_node) = data_result_node {
                Some(NodeInfo {
                    leaf_or_not: if data_result_node.children.is_empty() {
                        LeafOrNot::Leaf
                    } else {
                        LeafOrNot::NotLeaf(data_result_node)
                    },
                    kind: DataResultKind::EntityPart,
                    default_open: filter_matcher.is_active()
                        || default_open_for_data_result(data_result_node.children.len()),
                })
            } else {
                // TODO(ab): what are the circumstances for this? Should we warn about it?
                None
            }
        };

        //
        // …then handle the node accordingly.
        //

        let result = node_info.and_then(|node_info| {
            let (is_this_a_match, children) = match node_info.leaf_or_not {
                LeafOrNot::Leaf => {
                    // Key insight: we only ever need to match the hierarchy from the leaf nodes.
                    // Non-leaf nodes know they are a match if any child remains after walking their
                    // subtree.

                    let highlights =
                        filter_matcher.match_path(hierarchy.iter().map(String::as_str));

                    let is_this_a_match = if let Some(highlights) = highlights {
                        hierarchy_highlights.merge(highlights);
                        true
                    } else {
                        false
                    };

                    (is_this_a_match, vec![])
                }

                LeafOrNot::NotLeaf(data_result_node) => {
                    let mut children = data_result_node
                        .children
                        .iter()
                        .filter_map(|child_handle| {
                            let child_node = query_result.tree.lookup_node(*child_handle);

                            debug_assert!(
                                child_node.is_some(),
                                "DataResultNode {data_result_node:?} has an invalid child"
                            );

                            Self::from_data_result_and_filter(
                                view_blueprint,
                                query_result,
                                &DataResultNodeOrPath::DataResultNode(child_node?),
                                projection,
                                hierarchy,
                                hierarchy_highlights,
                                filter_matcher,
                            )
                        })
                        .collect_vec();

                    // This is needed because `DataResultNode` stores children in a `SmallVec`, offering
                    // no guarantees about ordering.
                    children.sort_by(|a, b| a.entity_path.cmp(&b.entity_path));

                    let is_this_a_match = !children.is_empty();

                    (is_this_a_match, children)
                }
            };

            is_this_a_match.then(|| {
                let highlight_sections =
                    hierarchy_highlights.remove(hierarchy.len().saturating_sub(1));

                // never highlight the placeholder
                let highlight_sections =
                    if node_info.kind == DataResultKind::OriginProjectionPlaceholder {
                        SmallVec::new()
                    } else {
                        highlight_sections
                            .map(Iterator::collect)
                            .unwrap_or_default()
                    };

                let visualizer_instruction_ids = data_result_node
                    .map(|node| {
                        node.data_result
                            .visualizer_instructions
                            .iter()
                            .map(|instr| instr.id)
                            .collect()
                    })
                    .unwrap_or_default();

                Self {
                    kind: node_info.kind,
                    entity_path,
                    visible,
                    visualizer_instruction_ids,
                    view_id: view_blueprint.id,
                    label,
                    highlight_sections,
                    default_open: node_info.default_open,
                    children,
                }
            })
        });

        if should_pop {
            hierarchy_highlights.remove(hierarchy.len().saturating_sub(1));
            hierarchy.pop();
        }

        result
    }

    pub fn visit<B>(
        &self,
        visitor: &mut impl FnMut(BlueprintTreeItem<'_>) -> VisitorControlFlow<B>,
    ) -> ControlFlow<B> {
        if visitor(BlueprintTreeItem::DataResult(self)).visit_children()? {
            for child in &self.children {
                child.visit(visitor)?;
            }
        }

        ControlFlow::Continue(())
    }

    pub fn item(&self) -> Item {
        Item::DataResult(DataResultInteractionAddress::from_entity_path(
            self.view_id,
            self.entity_path.clone(),
        ))
    }

    pub fn instance_path(&self) -> InstancePath {
        self.entity_path.clone().into()
    }

    /// Update the visibility of this data result.
    pub fn update_visibility(&self, ctx: &ViewerContext<'_>, visible: bool) {
        let query_result = ctx.lookup_query_result(self.view_id);
        let result_tree = &query_result.tree;
        if let Some(data_result) = result_tree.lookup_result_by_path(self.entity_path.hash()) {
            data_result.save_visible(ctx, &query_result.tree, visible);
        }
    }

    /// Remove this data result from the view.
    pub fn remove_data_result_from_view(
        &self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
    ) {
        if let Some(view_blueprint) = viewport_blueprint.view(&self.view_id) {
            view_blueprint
                .contents
                .remove_subtree_and_matching_rules(ctx, self.entity_path.clone());
        }
    }
}

/// If a group or view has a total of this number of elements, show its subtree by default?
//TODO(ab): taken from legacy implementation, does it still make sense?
fn default_open_for_data_result(num_children: usize) -> bool {
    2 <= num_children && num_children <= 3
}

// ---

/// Wrapper structure used for the closure of [`BlueprintTreeData::visit`] and friends.
#[derive(Debug, Clone, Copy)]
pub enum BlueprintTreeItem<'a> {
    Container(&'a ContainerData),
    View(&'a ViewData),
    DataResult(&'a DataResultData),
}

impl BlueprintTreeItem<'_> {
    pub fn item(&self) -> Item {
        match self {
            BlueprintTreeItem::Container(container_data) => container_data.item(),
            BlueprintTreeItem::View(view_data) => view_data.item(),
            BlueprintTreeItem::DataResult(data_result_data) => data_result_data.item(),
        }
    }

    pub fn default_open(&self) -> bool {
        match self {
            BlueprintTreeItem::Container(container_data) => container_data.default_open,
            BlueprintTreeItem::View(view_data) => view_data.default_open,
            BlueprintTreeItem::DataResult(data_result_data) => data_result_data.default_open,
        }
    }

    pub fn is_open(&self, ctx: &egui::Context, collapse_scope: CollapseScope) -> bool {
        collapse_scope.item(self.item()).is_some_and(|collapse_id| {
            collapse_id
                .is_open(ctx)
                .unwrap_or_else(|| self.default_open())
        })
    }
}
