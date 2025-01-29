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

use itertools::Itertools;
use smallvec::SmallVec;

use re_entity_db::InstancePath;
use re_log_types::external::re_types_core::ViewClassIdentifier;
use re_log_types::{EntityPath, EntityPathPart};
use re_types::blueprint::components::Visible;
use re_ui::filter_widget::FilterMatcher;
use re_viewer_context::{
    CollapseScope, ContainerId, Contents, ContentsName, DataQueryResult, DataResultNode, Item,
    ViewId, ViewerContext, VisitorControlFlow,
};
use re_viewport_blueprint::{ContainerBlueprint, ViewBlueprint, ViewportBlueprint};

use crate::data_result_node_or_path::DataResultNodeOrPath;

/// Top-level blueprint tree structure.
#[derive(Debug, Default)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct ContainerData {
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
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewData {
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
        let origin_tree = DataResultData::from_data_result_and_filter(
            ctx,
            view_blueprint,
            query_result,
            &DataResultNodeOrPath::from_path_lookup(result_tree, &view_blueprint.space_origin),
            false,
            &mut hierarchy,
            filter_matcher,
            false,
        );

        debug_assert!(hierarchy.is_empty());
        hierarchy.clear();

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
                let projection_tree = DataResultData::from_data_result_and_filter(
                    ctx,
                    view_blueprint,
                    query_result,
                    &DataResultNodeOrPath::DataResultNode(node),
                    true,
                    &mut hierarchy,
                    filter_matcher,
                    false,
                );

                debug_assert!(hierarchy.is_empty());
                hierarchy.clear();

                projection_tree
            })
            .collect_vec();

        if origin_tree.is_none() && projection_trees.is_empty() {
            return None;
        }

        let default_open = filter_matcher.is_active()
            || origin_tree.as_ref().map_or(true, |data_result_data| {
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
#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct DataResultData {
    pub kind: DataResultKind,
    pub entity_path: EntityPath,
    pub visible: bool,

    pub view_id: ViewId,

    /// Label that should be used for display.
    ///
    /// This typically corresponds to `entity_path.last().ui_string()` but covers corner cases.
    pub label: String,

    /// The sections within the label that correspond to a filter match and should thus be
    /// highlighted.
    pub highlight_sections: SmallVec<[Range<usize>; 1]>,

    pub default_open: bool,
    pub children: Vec<DataResultData>,
}

impl DataResultData {
    #[allow(clippy::too_many_arguments)]
    fn from_data_result_and_filter(
        ctx: &ViewerContext<'_>,
        view_blueprint: &ViewBlueprint,
        query_result: &DataQueryResult,
        data_result_or_path: &DataResultNodeOrPath<'_>,
        projection: bool,
        hierarchy: &mut Vec<EntityPathPart>,
        filter_matcher: &FilterMatcher,
        mut is_already_a_match: bool,
    ) -> Option<Self> {
        re_tracing::profile_function!();

        // Early out.
        if filter_matcher.matches_nothing() {
            return None;
        }

        let entity_path = data_result_or_path.path().clone();
        let data_result_node = data_result_or_path.data_result_node();
        let visible = data_result_node.is_some_and(|node| node.data_result.is_visible(ctx));

        let (label, should_pop) = if let Some(entity_part) = entity_path.last() {
            hierarchy.push(entity_part.clone());
            (entity_part.ui_string(), true)
        } else {
            ("/ (root)".to_owned(), false)
        };

        //
        // Filtering
        //

        // TODO(ab): we're currently only matching on the last part of `hierarchy`. Technically,
        // this means that `hierarchy` is not needed at all. It will however be needed when we match
        // across multiple parts, so it's good to have it already.
        let (entity_part_matches, highlight_sections) = if filter_matcher.matches_everything() {
            // fast path (filter is inactive)
            (true, SmallVec::new())
        } else if let Some(entity_part) = hierarchy.last() {
            // Nominal case of matching the hierarchy.
            if let Some(match_sections) = filter_matcher.find_matches(&entity_part.ui_string()) {
                (true, match_sections.collect())
            } else {
                (false, SmallVec::new())
            }
        } else {
            // `entity_path` is the root, it can never match anything
            (false, SmallVec::new())
        };

        // We want to keep entire branches if a single of its node matches. So we must propagate the
        // "matched" state so we can make the right call when we reach leaf nodes.
        is_already_a_match |= entity_part_matches;

        //
        // "Nominal" data result node (extracted for deduplication).
        //

        let view_id = view_blueprint.id;
        let mut from_data_result_node = |data_result_node: &DataResultNode,
                                         highlight_sections: SmallVec<_>,
                                         entity_path: EntityPath,
                                         label,
                                         default_open| {
            let mut children = data_result_node
                .children
                .iter()
                .filter_map(|child_handle| {
                    let child_node = query_result.tree.lookup_node(*child_handle);

                    debug_assert!(
                        child_node.is_some(),
                        "DataResultNode {data_result_node:?} has an invalid child"
                    );

                    child_node.and_then(|child_node| {
                        Self::from_data_result_and_filter(
                            ctx,
                            view_blueprint,
                            query_result,
                            &DataResultNodeOrPath::DataResultNode(child_node),
                            projection,
                            hierarchy,
                            filter_matcher,
                            is_already_a_match,
                        )
                    })
                })
                .collect_vec();

            children.sort_by(|a, b| a.entity_path.cmp(&b.entity_path));

            (is_already_a_match || !children.is_empty()).then(|| Self {
                kind: DataResultKind::EntityPart,
                entity_path,
                visible,
                view_id,
                label,
                highlight_sections,
                default_open,
                children,
            })
        };

        //
        // Handle all situations
        //

        let result = if projection {
            //  projections are collapsed by default
            let default_open = filter_matcher.is_active();

            if entity_path == view_blueprint.space_origin {
                is_already_a_match.then(|| Self {
                    kind: DataResultKind::OriginProjectionPlaceholder,
                    entity_path,
                    visible,
                    view_id,
                    label,
                    highlight_sections,
                    default_open,
                    children: vec![],
                })
            } else if let Some(data_result_node) = data_result_node {
                from_data_result_node(
                    data_result_node,
                    highlight_sections,
                    entity_path,
                    label,
                    filter_matcher.is_active(),
                )
            } else {
                // TODO(ab): what are the circumstances for this? Should we warn about it?
                None
            }
        } else {
            // empty origin case
            if entity_path == view_blueprint.space_origin && data_result_node.is_none() {
                is_already_a_match.then(|| Self {
                    kind: DataResultKind::EmptyOriginPlaceholder,
                    entity_path,
                    visible,
                    view_id,
                    label,
                    highlight_sections,
                    default_open: false, // not hierarchical anyway
                    children: vec![],
                })
            } else if let Some(data_result_node) = data_result_node {
                let default_open = filter_matcher.is_active()
                    || default_open_for_data_result(data_result_node.children.len());

                from_data_result_node(
                    data_result_node,
                    highlight_sections,
                    entity_path,
                    label,
                    default_open,
                )
            } else {
                // TODO(ab): what are the circumstances for this? Should we warn about it?
                None
            }
        };

        if should_pop {
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
        Item::DataResult(self.view_id, self.instance_path())
    }

    pub fn instance_path(&self) -> InstancePath {
        self.entity_path.clone().into()
    }

    /// Update the visibility of this data result.
    pub fn update_visibility(&self, ctx: &ViewerContext<'_>, visible: bool) {
        let query_result = ctx.lookup_query_result(self.view_id);
        let result_tree = &query_result.tree;
        if let Some(data_result) = result_tree.lookup_result_by_path(&self.entity_path) {
            data_result.save_recursive_override_or_clear_if_redundant(
                ctx,
                &query_result.tree,
                &Visible::from(visible),
            );
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

    pub fn is_open(&self, ctx: &egui::Context, collapse_scope: CollapseScope) -> Option<bool> {
        collapse_scope.item(self.item()).map(|collapse_id| {
            collapse_id
                .is_open(ctx)
                .unwrap_or_else(|| self.default_open())
        })
    }
}
