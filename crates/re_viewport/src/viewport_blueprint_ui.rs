use egui::{Response, Ui};

use itertools::Itertools;

use re_entity_db::InstancePath;
use re_log_types::EntityPath;
use re_log_types::EntityPathRule;
use re_space_view::{SpaceViewBlueprint, SpaceViewName};
use re_ui::{drag_and_drop::DropTarget, list_item::ListItem, ReUi};
use re_viewer_context::DataResultTree;
use re_viewer_context::{
    ContainerId, DataQueryResult, DataResultNode, HoverHighlight, Item, SpaceViewId, ViewerContext,
};

use crate::{container::Contents, context_menu_ui_for_item, SelectionUpdateBehavior, Viewport};

/// The style to use for displaying this space view name in the UI.
pub fn space_view_name_style(name: &SpaceViewName) -> re_ui::LabelStyle {
    match name {
        SpaceViewName::Named(_) => re_ui::LabelStyle::Normal,
        SpaceViewName::Placeholder(_) => re_ui::LabelStyle::Unnamed,
    }
}

enum DataResultNodeOrPath<'a> {
    Path(&'a EntityPath),
    DataResultNode(&'a DataResultNode),
}

impl<'a> DataResultNodeOrPath<'a> {
    fn from_path_lookup(result_tree: &'a DataResultTree, path: &'a EntityPath) -> Self {
        result_tree
            .lookup_node_by_path(path)
            .map_or(DataResultNodeOrPath::Path(path), |node| {
                DataResultNodeOrPath::DataResultNode(node)
            })
    }

    fn path(&self) -> &'a EntityPath {
        match self {
            DataResultNodeOrPath::Path(path) => path,
            DataResultNodeOrPath::DataResultNode(node) => &node.data_result.entity_path,
        }
    }

    fn data_result_node(&self) -> Option<&'a DataResultNode> {
        match self {
            DataResultNodeOrPath::Path(_) => None,
            DataResultNodeOrPath::DataResultNode(node) => Some(node),
        }
    }
}

impl Viewport<'_, '_> {
    /// Show the blueprint panel tree view.
    pub fn tree_ui(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        re_tracing::profile_function!();

        egui::ScrollArea::both()
            .id_source("blueprint_tree_scroll_area")
            .auto_shrink([true, false])
            .show(ui, |ui| {
                ctx.re_ui.panel_content(ui, |_, ui| {
                    self.root_container_tree_ui(ctx, ui);

                    let empty_space_response =
                        ui.allocate_response(ui.available_size(), egui::Sense::click());

                    // clear selection upon clicking on empty space
                    if empty_space_response.clicked() {
                        ctx.selection_state().clear_current();
                    }

                    // handle drag and drop interaction on empty space
                    self.handle_empty_space_drag_and_drop_interaction(
                        ui,
                        empty_space_response.rect,
                    );
                });
            });
    }

    /// If a group or spaceview has a total of this number of elements, show its subtree by default?
    fn default_open_for_data_result(group: &DataResultNode) -> bool {
        let num_children = group.children.len();
        2 <= num_children && num_children <= 3
    }

    fn contents_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        contents: &Contents,
        parent_visible: bool,
    ) {
        match contents {
            Contents::Container(container_id) => {
                self.container_tree_ui(ctx, ui, container_id, parent_visible);
            }
            Contents::SpaceView(space_view_id) => {
                self.space_view_entry_ui(ctx, ui, space_view_id, parent_visible);
            }
        };
    }

    /// Display the root container.
    ///
    /// The root container is different from other containers in that it cannot be removed or dragged, and it cannot be
    /// collapsed, so it's drawn without a collapsing triangle..
    fn root_container_tree_ui(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        let Some(container_id) = self.blueprint.root_container else {
            // nothing to draw if there is no root container
            return;
        };

        let Some(container_blueprint) = self.blueprint.containers.get(&container_id) else {
            re_log::warn_once!("Cannot find root container {container_id}");
            return;
        };

        let item = Item::Container(container_id);

        let item_response = ListItem::new(
            ctx.re_ui,
            format!("Viewport ({:?})", container_blueprint.container_kind),
        )
        .selected(ctx.selection().contains_item(&item))
        .draggable(false)
        .drop_target_style(self.state.is_candidate_drop_parent_container(&container_id))
        .label_style(re_ui::LabelStyle::Unnamed)
        .with_icon(crate::icon_for_container_kind(
            &container_blueprint.container_kind,
        ))
        .show(ui);

        for child in &container_blueprint.contents {
            self.contents_ui(ctx, ui, child, true);
        }

        context_menu_ui_for_item(
            ctx,
            self.blueprint,
            &item,
            &item_response,
            SelectionUpdateBehavior::UseSelection,
        );
        ctx.select_hovered_on_click(&item_response, item);

        self.handle_root_container_drag_and_drop_interaction(
            ui,
            Contents::Container(container_id),
            &item_response,
        );
    }

    fn container_tree_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        container_id: &ContainerId,
        parent_visible: bool,
    ) {
        let item = Item::Container(*container_id);
        let content = Contents::Container(*container_id);

        let Some(container_blueprint) = self.blueprint.containers.get(container_id) else {
            re_log::warn_once!("Ignoring unknown container {container_id}");
            return;
        };

        let mut visible = container_blueprint.visible;
        let container_visible = visible && parent_visible;

        let default_open = true;

        let re_ui::list_item::ShowCollapsingResponse {
            item_response: response,
            body_response,
        } = ListItem::new(
            ctx.re_ui,
            format!("{:?}", container_blueprint.container_kind),
        )
        .subdued(!container_visible)
        .selected(ctx.selection().contains_item(&item))
        .draggable(true)
        .drop_target_style(self.state.is_candidate_drop_parent_container(container_id))
        .label_style(re_ui::LabelStyle::Unnamed)
        .with_icon(crate::icon_for_container_kind(
            &container_blueprint.container_kind,
        ))
        .with_buttons(|re_ui, ui| {
            let vis_response = visibility_button_ui(re_ui, ui, parent_visible, &mut visible);

            let remove_response = remove_button_ui(re_ui, ui, "Remove container");
            if remove_response.clicked() {
                self.blueprint.mark_user_interaction(ctx);
                self.blueprint.remove_contents(content);
            }

            remove_response | vis_response
        })
        .show_collapsing(ui, ui.id().with(container_id), default_open, |_, ui| {
            for child in &container_blueprint.contents {
                self.contents_ui(ctx, ui, child, container_visible);
            }
        });

        context_menu_ui_for_item(
            ctx,
            self.blueprint,
            &item,
            &response,
            SelectionUpdateBehavior::UseSelection,
        );
        ctx.select_hovered_on_click(&response, item);

        self.blueprint
            .set_content_visibility(ctx, &content, visible);

        self.handle_drag_and_drop_interaction(
            ctx,
            ui,
            content,
            &response,
            body_response.as_ref().map(|r| &r.response),
        );
    }

    fn space_view_entry_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view_id: &SpaceViewId,
        container_visible: bool,
    ) {
        let Some(space_view) = self.blueprint.space_views.get(space_view_id) else {
            re_log::warn_once!("Bug: asked to show a ui for a Space View that doesn't exist");
            return;
        };
        debug_assert_eq!(space_view.id, *space_view_id);

        let query_result = ctx.lookup_query_result(space_view.query_id());
        let result_tree = &query_result.tree;

        let mut visible = space_view.visible;
        let space_view_visible = visible && container_visible;
        let item = Item::SpaceView(space_view.id);

        let root_node = result_tree.root_node();

        // empty space views should display as open by default to highlight the fact that they are empty
        let default_open = root_node.map_or(true, Self::default_open_for_data_result);

        let collapsing_header_id = ui.id().with(space_view.id);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let space_view_name = space_view.display_name_or_default();

        let re_ui::list_item::ShowCollapsingResponse {
            item_response: mut response,
            body_response,
        } = ListItem::new(ctx.re_ui, space_view_name.as_ref())
            .label_style(space_view_name_style(&space_view_name))
            .with_icon(space_view.class(ctx.space_view_class_registry).icon())
            .selected(ctx.selection().contains_item(&item))
            .draggable(true)
            .subdued(!space_view_visible)
            .force_hovered(is_item_hovered)
            .with_buttons(|re_ui, ui| {
                let vis_response = visibility_button_ui(re_ui, ui, container_visible, &mut visible);

                let response = remove_button_ui(re_ui, ui, "Remove Space View from the Viewport");
                if response.clicked() {
                    self.blueprint.mark_user_interaction(ctx);
                    self.blueprint
                        .remove_contents(Contents::SpaceView(*space_view_id));
                }

                response | vis_response
            })
            .show_collapsing(ui, collapsing_header_id, default_open, |_, ui| {
                // Always show the origin hierarchy first.
                Self::space_view_entity_hierarchy_ui(
                    ctx,
                    ui,
                    query_result,
                    &DataResultNodeOrPath::from_path_lookup(result_tree, &space_view.space_origin),
                    space_view,
                    space_view_visible,
                );

                // Show 'projections' if there's any items that weren't part of the tree under origin.
                let mut projections = Vec::new();
                result_tree.visit(&mut |node| {
                    if !node
                        .data_result
                        .entity_path
                        .starts_with(&space_view.space_origin)
                    {
                        projections.push(node);
                        false
                    } else {
                        true
                    }
                });
                if !projections.is_empty() {
                    ui.label(egui::RichText::new("Projections:").italics());

                    for projection in projections {
                        Self::space_view_entity_hierarchy_ui(
                            ctx,
                            ui,
                            query_result,
                            &DataResultNodeOrPath::DataResultNode(projection),
                            space_view,
                            space_view_visible,
                        );
                    }
                }
            });

        response = response.on_hover_text("Space View");

        if response.clicked() {
            self.blueprint.focus_tab(space_view.id);
        }

        context_menu_ui_for_item(
            ctx,
            self.blueprint,
            &item,
            &response,
            SelectionUpdateBehavior::UseSelection,
        );
        ctx.select_hovered_on_click(&response, item);

        let content = Contents::SpaceView(*space_view_id);

        self.blueprint
            .set_content_visibility(ctx, &content, visible);
        self.handle_drag_and_drop_interaction(
            ctx,
            ui,
            content,
            &response,
            body_response.as_ref().map(|r| &r.response),
        );
    }

    fn space_view_entity_hierarchy_ui(
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        query_result: &DataQueryResult,
        node_or_path: &DataResultNodeOrPath<'_>,
        space_view: &SpaceViewBlueprint,
        space_view_visible: bool,
    ) {
        let store = ctx.entity_db.store();

        let entity_path = node_or_path.path();
        let data_result_node = node_or_path.data_result_node();

        let item = Item::InstancePath(
            Some(space_view.id),
            InstancePath::entity_splat(entity_path.clone()),
        );
        let is_selected = ctx.selection().contains_item(&item);
        let is_item_hovered =
            ctx.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        let visible =
            data_result_node.map_or(false, |n| n.data_result.accumulated_properties().visible);
        let mut recursive_properties = data_result_node
            .and_then(|n| n.data_result.recursive_properties())
            .cloned()
            .unwrap_or_default();

        let item_label = if entity_path.is_root() {
            "/ (root)".to_owned()
        } else {
            entity_path
                .iter()
                .last()
                .map_or("unknown".to_owned(), |e| e.ui_string())
        };
        let item_label = if ctx.entity_db.is_known_entity(entity_path) {
            egui::RichText::new(item_label)
        } else {
            ctx.re_ui.warning_text(item_label)
        };

        let subdued = !space_view_visible
            || !visible
            || data_result_node.map_or(true, |n| {
                n.data_result.visualizers.is_empty() && n.children.is_empty()
            });

        let list_item = ListItem::new(ctx.re_ui, item_label)
            .selected(is_selected)
            .with_icon(&re_ui::icons::ENTITY)
            .subdued(subdued)
            .force_hovered(is_item_hovered)
            .with_buttons(|re_ui: &_, ui: &mut egui::Ui| {
                let vis_response = visibility_button_ui(
                    re_ui,
                    ui,
                    space_view_visible,
                    &mut recursive_properties.visible,
                );

                let response = remove_button_ui(
                    re_ui,
                    ui,
                    "Remove Group and all its children from the Space View",
                );
                if response.clicked() {
                    space_view.add_entity_exclusion(
                        ctx,
                        EntityPathRule::including_subtree(entity_path.clone()),
                    );
                }

                response | vis_response
            });

        // If there's any children on the data result nodes, show them, otherwise we're good with this list item as is.
        let has_children = data_result_node.map_or(false, |n| !n.children.is_empty());
        let response = if let (true, Some(node)) = (has_children, data_result_node) {
            let default_open = Self::default_open_for_data_result(node);
            let response = list_item
                .show_collapsing(
                    ui,
                    ui.id().with(&node.data_result.entity_path),
                    default_open,
                    |_, ui| {
                        for child in node.children.iter().sorted_by_key(|c| {
                            query_result
                                .tree
                                .lookup_result(**c)
                                .map_or(&space_view.space_origin, |c| &c.entity_path)
                        }) {
                            let Some(child_node) = query_result.tree.lookup_node(*child) else {
                                debug_assert!(
                                    false,
                                    "DataResultNode {node:?} has an invalid child"
                                );
                                continue;
                            };

                            Self::space_view_entity_hierarchy_ui(
                                ctx,
                                ui,
                                query_result,
                                &DataResultNodeOrPath::DataResultNode(child_node),
                                space_view,
                                space_view_visible,
                            );
                        }
                    },
                )
                .item_response;

            node.data_result
                .save_recursive_override(ctx, Some(recursive_properties));

            response
        } else {
            list_item.show(ui)
        };

        let response = response.on_hover_ui(|ui| {
            let query = ctx.current_query();
            re_data_ui::item_ui::entity_hover_card_ui(ui, ctx, &query, store, entity_path);
        });
        ctx.select_hovered_on_click(&response, item);
    }

    pub fn add_new_spaceview_button_ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        if ctx
            .re_ui
            .small_icon_button(ui, &re_ui::icons::ADD)
            .on_hover_text("Add a new Space View or Container")
            .clicked()
        {
            // If a single container is selected, we use it as target. Otherwise, we target the
            // root container.
            let target_container_id =
                if let Some(Item::Container(container_id)) = ctx.selection().single_item() {
                    Some(*container_id)
                } else {
                    self.blueprint.root_container
                };

            if let Some(target_container_id) = target_container_id {
                self.show_add_space_view_or_container_modal(target_container_id);
            }
        }
    }

    // ----------------------------------------------------------------------------
    // drag and drop support

    fn handle_root_container_drag_and_drop_interaction(
        &self,
        ui: &egui::Ui,
        contents: Contents,
        response: &egui::Response,
    ) {
        //
        // check if a drag is in progress and set the cursor accordingly
        //

        let Some(dragged_item_id) = egui::DragAndDrop::payload(ui.ctx()).map(|payload| *payload)
        else {
            // nothing is being dragged, so nothing to do
            return;
        };

        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        //
        // find the drop target
        //

        // Prepare the item description structure needed by `find_drop_target`. Here, we use
        // `Contents` for the "ItemId" generic type parameter.
        let item_desc = re_ui::drag_and_drop::ItemContext {
            id: contents,
            item_kind: re_ui::drag_and_drop::ItemKind::RootContainer,
            previous_container_id: None,
        };

        let drop_target = re_ui::drag_and_drop::find_drop_target(
            ui,
            &item_desc,
            response.rect,
            None,
            ReUi::list_item_height(),
        );

        if let Some(drop_target) = drop_target {
            self.handle_drop_target(ui, dragged_item_id, &drop_target);
        }
    }

    fn handle_drag_and_drop_interaction(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &egui::Ui,
        contents: Contents,
        response: &egui::Response,
        body_response: Option<&egui::Response>,
    ) {
        //
        // initiate drag and force single-selection
        //

        if response.drag_started() {
            ctx.selection_state().set_selection(contents.as_item());
            egui::DragAndDrop::set_payload(ui.ctx(), contents);
        }

        //
        // check if a drag is in progress and set the cursor accordingly
        //

        let Some(dragged_item_id) = egui::DragAndDrop::payload(ui.ctx()).map(|payload| *payload)
        else {
            // nothing is being dragged, so nothing to do
            return;
        };

        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        //
        // find our parent, our position within parent, and the previous container (if any)
        //

        let Some((parent_container_id, position_index_in_parent)) =
            self.blueprint.find_parent_and_position_index(&contents)
        else {
            return;
        };

        let previous_container = if position_index_in_parent > 0 {
            self.blueprint
                .container(&parent_container_id)
                .map(|container| container.contents[position_index_in_parent - 1])
                .filter(|contents| matches!(contents, Contents::Container(_)))
        } else {
            None
        };

        //
        // find the drop target
        //

        // Prepare the item description structure needed by `find_drop_target`. Here, we use
        // `Contents` for the "ItemId" generic type parameter.

        let item_desc = re_ui::drag_and_drop::ItemContext {
            id: contents,
            item_kind: match contents {
                Contents::Container(_) => re_ui::drag_and_drop::ItemKind::Container {
                    parent_id: Contents::Container(parent_container_id),
                    position_index_in_parent,
                },
                Contents::SpaceView(_) => re_ui::drag_and_drop::ItemKind::Leaf {
                    parent_id: Contents::Container(parent_container_id),
                    position_index_in_parent,
                },
            },
            previous_container_id: previous_container,
        };

        let drop_target = re_ui::drag_and_drop::find_drop_target(
            ui,
            &item_desc,
            response.rect,
            body_response.map(|r| r.rect),
            ReUi::list_item_height(),
        );

        if let Some(drop_target) = drop_target {
            self.handle_drop_target(ui, dragged_item_id, &drop_target);
        }
    }

    fn handle_empty_space_drag_and_drop_interaction(&self, ui: &egui::Ui, empty_space: egui::Rect) {
        //
        // check if a drag is in progress and set the cursor accordingly
        //

        let Some(dragged_item_id) = egui::DragAndDrop::payload(ui.ctx()).map(|payload| *payload)
        else {
            // nothing is being dragged, so nothing to do
            return;
        };

        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        //
        // prepare a drop target corresponding to "insert last in root container"
        //
        // TODO(ab): this is a rather primitive behavior. Ideally we should allow dropping in the last container based
        //           on the horizontal position of the cursor.

        let Some(root_container_id) = self.blueprint.root_container else {
            return;
        };

        if ui.rect_contains_pointer(empty_space) {
            let drop_target = re_ui::drag_and_drop::DropTarget::new(
                empty_space.x_range(),
                empty_space.top(),
                Contents::Container(root_container_id),
                usize::MAX,
            );

            self.handle_drop_target(ui, dragged_item_id, &drop_target);
        }
    }

    fn handle_drop_target(
        &self,
        ui: &Ui,
        dragged_item_id: Contents,
        drop_target: &DropTarget<Contents>,
    ) {
        // We cannot allow the target location to be "inside" the dragged item, because that would amount moving
        // myself inside of me.
        if let Contents::Container(dragged_container_id) = &dragged_item_id {
            if self
                .blueprint
                .is_contents_in_container(&drop_target.target_parent_id, dragged_container_id)
            {
                return;
            }
        }

        ui.painter().hline(
            drop_target.indicator_span_x,
            drop_target.indicator_position_y,
            (2.0, egui::Color32::WHITE),
        );

        let Contents::Container(target_container_id) = drop_target.target_parent_id else {
            // this shouldn't append
            return;
        };

        if ui.input(|i| i.pointer.any_released()) {
            self.blueprint.move_contents(
                dragged_item_id,
                target_container_id,
                drop_target.target_position_index,
            );

            egui::DragAndDrop::clear_payload(ui.ctx());
        } else {
            self.blueprint.set_drop_target(&target_container_id);
        }
    }
}

// ----------------------------------------------------------------------------

fn remove_button_ui(re_ui: &ReUi, ui: &mut Ui, tooltip: &str) -> Response {
    re_ui
        .small_icon_button(ui, &re_ui::icons::REMOVE)
        .on_hover_text(tooltip)
}

fn visibility_button_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    enabled: bool,
    visible: &mut bool,
) -> egui::Response {
    ui.set_enabled(enabled);
    re_ui
        .visibility_toggle_button(ui, visible)
        .on_hover_text("Toggle visibility")
        .on_disabled_hover_text("A parent is invisible")
}
