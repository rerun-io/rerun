use itertools::Itertools;
use once_cell::sync::OnceCell;

use re_entity_db::InstancePath;
use re_viewer_context::{ContainerId, Item, ItemCollection, SpaceViewId, ViewerContext};

use crate::{ContainerBlueprint, Contents, ViewportBlueprint};

mod actions;
mod sub_menu;

use actions::{
    add_container::AddContainerAction,
    add_entities_to_new_space_view::AddEntitiesToNewSpaceViewAction,
    add_space_view::AddSpaceViewAction,
    clone_space_view::CloneSpaceViewAction,
    collapse_expand_all::CollapseExpandAllAction,
    move_contents_to_new_container::MoveContentsToNewContainerAction,
    remove::RemoveAction,
    show_hide::{HideAction, ShowAction},
};
use sub_menu::SubMenu;

/// Controls how [`context_menu_ui_for_item`] should handle the current selection state.
#[derive(Debug, Clone, Copy)]
pub enum SelectionUpdateBehavior {
    /// If part of the current selection, use it. Otherwise, set selection to clicked item.
    UseSelection,

    /// Discard the current selection state and set the selection to the clicked item.
    OverrideSelection,

    /// Ignore the current selection and consider only the clicked item.
    Ignore,
}

/// Display a context menu for the provided [`Item`]
pub fn context_menu_ui_for_item(
    ctx: &ViewerContext<'_>,
    viewport_blueprint: &ViewportBlueprint,
    item: &Item,
    item_response: &egui::Response,
    selection_update_behavior: SelectionUpdateBehavior,
) {
    item_response.context_menu(|ui| {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.close_menu();
            return;
        }

        let mut show_context_menu = |selection: &ItemCollection| {
            let context_menu_ctx = ContextMenuContext {
                viewer_context: ctx,
                viewport_blueprint,
                egui_context: ui.ctx().clone(),
                selection,
                clicked_item: item,
            };
            show_context_menu_for_selection(&context_menu_ctx, ui);
        };

        // handle selection
        match selection_update_behavior {
            SelectionUpdateBehavior::UseSelection => {
                if !ctx.selection().contains_item(item) {
                    // When the context menu is triggered open, we check if we're part of the selection,
                    // and, if not, we update the selection to include only the item that was clicked.
                    if item_response.hovered() && item_response.secondary_clicked() {
                        ctx.selection_state().set_selection(item.clone());

                        show_context_menu(&ItemCollection::from(item.clone()));
                    } else {
                        show_context_menu(ctx.selection());
                    }
                } else {
                    show_context_menu(ctx.selection());
                }
            }

            SelectionUpdateBehavior::OverrideSelection => {
                if item_response.secondary_clicked() {
                    ctx.selection_state().set_selection(item.clone());
                }

                show_context_menu(&ItemCollection::from(item.clone()));
            }

            SelectionUpdateBehavior::Ignore => {
                show_context_menu(&ItemCollection::from(item.clone()));
            }
        };
    });
}

/// Returns the (statically-defined) list of action, grouped in sections.
///
/// Sections are group of actions that should be displayed together, with a separator displayed
/// between sections.
fn action_list(
    ctx: &ViewerContext<'_>,
) -> &'static Vec<Vec<Box<dyn ContextMenuAction + Sync + Send>>> {
    static CONTEXT_MENU_ACTIONS: OnceCell<Vec<Vec<Box<dyn ContextMenuAction + Sync + Send>>>> =
        OnceCell::new();

    CONTEXT_MENU_ACTIONS.get_or_init(|| {
        vec![
            vec![
                Box::new(ShowAction),
                Box::new(HideAction),
                Box::new(RemoveAction),
            ],
            vec![
                Box::new(CollapseExpandAllAction::ExpandAll),
                Box::new(CollapseExpandAllAction::CollapseAll),
            ],
            vec![Box::new(CloneSpaceViewAction)],
            vec![
                Box::new(SubMenu {
                    label: "Add container".to_owned(),
                    actions: vec![
                        Box::new(AddContainerAction(egui_tiles::ContainerKind::Tabs)),
                        Box::new(AddContainerAction(egui_tiles::ContainerKind::Horizontal)),
                        Box::new(AddContainerAction(egui_tiles::ContainerKind::Vertical)),
                        Box::new(AddContainerAction(egui_tiles::ContainerKind::Grid)),
                    ],
                }),
                Box::new(SubMenu {
                    label: "Add Space View".to_owned(),
                    actions: ctx
                        .space_view_class_registry
                        .iter_registry()
                        .sorted_by_key(|entry| entry.class.display_name())
                        .map(|entry| {
                            Box::new(AddSpaceViewAction(entry.identifier))
                                as Box<dyn ContextMenuAction + Sync + Send>
                        })
                        .collect(),
                }),
            ],
            vec![Box::new(SubMenu {
                label: "Move to new container".to_owned(),
                actions: vec![
                    Box::new(MoveContentsToNewContainerAction(
                        egui_tiles::ContainerKind::Tabs,
                    )),
                    Box::new(MoveContentsToNewContainerAction(
                        egui_tiles::ContainerKind::Horizontal,
                    )),
                    Box::new(MoveContentsToNewContainerAction(
                        egui_tiles::ContainerKind::Vertical,
                    )),
                    Box::new(MoveContentsToNewContainerAction(
                        egui_tiles::ContainerKind::Grid,
                    )),
                ],
            })],
            vec![Box::new(AddEntitiesToNewSpaceViewAction)],
        ]
    })
}

/// Display every action that accepts the provided selection.
fn show_context_menu_for_selection(ctx: &ContextMenuContext<'_>, ui: &mut egui::Ui) {
    let mut should_display_separator = false;
    for action_section in action_list(ctx.viewer_context) {
        let mut any_action_displayed = false;

        for action in action_section {
            if !action.supports_selection(ctx) {
                continue;
            }

            any_action_displayed = true;

            if should_display_separator {
                ui.separator();
                should_display_separator = false;
            }

            let response = action.ui(ctx, ui);
            if response.clicked() {
                ui.close_menu();
            }
        }

        should_display_separator |= any_action_displayed;
    }

    // If anything was shown, then `should_display_separator` has to be true. We can therefore
    // recycle this flag for the empty menu message.
    if !should_display_separator {
        ui.label(egui::RichText::from("No action available for the current selection").italics());
    }
}

/// Context information provided to context menu actions
struct ContextMenuContext<'a> {
    viewer_context: &'a ViewerContext<'a>,
    viewport_blueprint: &'a ViewportBlueprint,
    egui_context: egui::Context,
    selection: &'a ItemCollection,
    clicked_item: &'a Item,
}

impl<'a> ContextMenuContext<'a> {
    /// Return the clicked item's parent container id and position within it.
    ///
    /// Valid only for space views, containers, and data results. For data results, the parent and
    /// position of the enclosing space view is considered.
    pub fn clicked_item_enclosing_container_id_and_position(&self) -> Option<(ContainerId, usize)> {
        match self.clicked_item {
            Item::SpaceView(space_view_id) | Item::DataResult(space_view_id, _) => {
                Some(Contents::SpaceView(*space_view_id))
            }
            Item::Container(container_id) => Some(Contents::Container(*container_id)),
            _ => None,
        }
        .and_then(|c: Contents| self.viewport_blueprint.find_parent_and_position_index(&c))
    }

    /// Return the clicked item's parent container and position within it.
    ///
    /// Valid only for space views, containers, and data results. For data results, the parent and
    /// position of the enclosing space view is considered.
    pub fn clicked_item_enclosing_container_and_position(
        &self,
    ) -> Option<(&'a ContainerBlueprint, usize)> {
        self.clicked_item_enclosing_container_id_and_position()
            .and_then(|(container_id, pos)| {
                self.viewport_blueprint
                    .container(&container_id)
                    .map(|container| (container, pos))
            })
    }
}

/// Context menu actions must implement this trait.
///
/// Actions must do three things, corresponding to three core methods:
/// 1. Decide if it can operate a given [`ItemCollection`] ([`Self::supports_selection`]).
/// 2. If so, draw some UI in the context menu ([`Self::ui`]).
/// 3. If clicked, actually process the [`ItemCollection`] ([`Self::process_selection`]).
///
/// For convenience, these core methods have default implementations which delegates to simpler
/// methods (see their respective docstrings). Implementor may either implement the core method for
/// complex cases, or one or more of the helper methods.
trait ContextMenuAction {
    /// Check if the action is able to operate on the provided selection.
    ///
    /// The default implementation delegates to [`Self::supports_multi_selection`] and
    /// [`Self::supports_item`].
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        if ctx.selection.len() > 1 && !self.supports_multi_selection(ctx) {
            return false;
        }

        ctx.selection
            .iter()
            .all(|(item, _)| self.supports_item(ctx, item))
    }

    /// Returns whether this action supports multi-selections.
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        false
    }

    /// Returns whether this action supports operation on a selection containing this [`Item`].
    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, _item: &Item) -> bool {
        false
    }

    // ---

    /// Draw the context menu UI for this action.
    ///
    /// The default implementation delegates to [`Self::label`].
    ///
    /// Note: this is run from inside a [`egui::Response.context_menu()`] closure and must call
    /// [`Self::process_selection`] when triggered by the user.
    fn ui(&self, ctx: &ContextMenuContext<'_>, ui: &mut egui::Ui) -> egui::Response {
        let label = self.label(ctx);
        let response = ui.button(label);
        if response.clicked() {
            self.process_selection(ctx);
        }
        response
    }

    // TODO(ab): return a `ListItem` to make those context menu nice to look at. This requires
    // changes to the context menu UI code to support full-span highlighting.
    /// Returns the label displayed by [`Self::ui`]'s default implementation.
    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        String::new()
    }

    // ---

    /// Process the provided [`ItemCollection`].
    ///
    /// The default implementation dispatches to [`Self::process_store_id`] and friends.
    fn process_selection(&self, ctx: &ContextMenuContext<'_>) {
        for (item, _) in ctx.selection.iter() {
            match item {
                Item::StoreId(store_id) => self.process_store_id(ctx, store_id),
                Item::ComponentPath(component_path) => {
                    self.process_component_path(ctx, component_path);
                }
                Item::SpaceView(space_view_id) => self.process_space_view(ctx, space_view_id),
                Item::InstancePath(instance_path) => self.process_instance_path(ctx, instance_path),
                Item::DataResult(space_view_id, instance_path) => {
                    self.process_data_result(ctx, space_view_id, instance_path);
                }
                Item::Container(container_id) => self.process_container(ctx, container_id),
            }
        }
    }

    /// Process a single recording.
    fn process_store_id(&self, _ctx: &ContextMenuContext<'_>, _store_id: &re_log_types::StoreId) {}

    /// Process a single container.
    fn process_container(&self, _ctx: &ContextMenuContext<'_>, _container_id: &ContainerId) {}

    /// Process a single space view.
    fn process_space_view(&self, _ctx: &ContextMenuContext<'_>, _space_view_id: &SpaceViewId) {}

    /// Process a single data result.
    fn process_data_result(
        &self,
        _ctx: &ContextMenuContext<'_>,
        _space_view_id: &SpaceViewId,
        _instance_path: &InstancePath,
    ) {
    }

    /// Process a single instance.
    fn process_instance_path(&self, _ctx: &ContextMenuContext<'_>, _instance_path: &InstancePath) {}

    /// Process a single component.
    fn process_component_path(
        &self,
        _ctx: &ContextMenuContext<'_>,
        _component_path: &re_log_types::ComponentPath,
    ) {
    }
}
