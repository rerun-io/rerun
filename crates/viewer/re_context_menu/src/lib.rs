//! Support crate for context menu and actions.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

use std::sync::OnceLock;

use egui::Popup;
use re_entity_db::InstancePath;
use re_log_types::TableId;
use re_ui::UiExt as _;
use re_viewer_context::{
    ContainerId, Contents, Item, ItemCollection, ItemContext, SystemCommand,
    SystemCommandSender as _, ViewId, ViewerContext,
};
use re_viewport_blueprint::{ContainerBlueprint, ViewportBlueprint};

mod actions;
pub mod collapse_expand;
mod sub_menu;

use actions::add_container::AddContainerAction;
use actions::add_entities_to_new_view::AddEntitiesToNewViewAction;
use actions::add_view::AddViewAction;
use actions::clone_view::CloneViewAction;
use actions::collapse_expand_all::CollapseExpandAllAction;
use actions::move_contents_to_new_container::MoveContentsToNewContainerAction;
use actions::remove::RemoveAction;
use actions::show_hide::{HideAction, ShowAction};
use actions::{CopyEntityPathToClipboard, TrackEntity};
use re_ui::menu::menu_style;
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
    context_menu_ui_for_item_with_context_impl(
        ctx,
        viewport_blueprint,
        item,
        None,
        item_response,
        selection_update_behavior,
    );
}

/// Display a context menu for the provided [`Item`]
pub fn context_menu_ui_for_item_with_context(
    ctx: &ViewerContext<'_>,
    viewport_blueprint: &ViewportBlueprint,
    item: &Item,
    item_context: ItemContext,
    item_response: &egui::Response,
    selection_update_behavior: SelectionUpdateBehavior,
) {
    context_menu_ui_for_item_with_context_impl(
        ctx,
        viewport_blueprint,
        item,
        Some(item_context),
        item_response,
        selection_update_behavior,
    );
}

fn context_menu_ui_for_item_with_context_impl(
    ctx: &ViewerContext<'_>,
    viewport_blueprint: &ViewportBlueprint,
    item: &Item,
    item_context: Option<ItemContext>,
    item_response: &egui::Response,
    selection_update_behavior: SelectionUpdateBehavior,
) {
    Popup::context_menu(item_response)
        .style(menu_style())
        .show(|ui| {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                ui.close();
                return;
            }

            let mut show_context_menu = |selection: &ItemCollection| {
                let context_menu_ctx = ContextMenuContext {
                    viewer_context: ctx,
                    viewport_blueprint,
                    selection,
                    clicked_item: item,
                };
                show_context_menu_for_selection(&context_menu_ctx, ui);
            };

            let item_collection = ItemCollection::from_items_and_context(std::iter::once((
                item.clone(),
                item_context,
            )));

            // handle selection
            match selection_update_behavior {
                SelectionUpdateBehavior::UseSelection => {
                    if !ctx.selection().contains_item(item) {
                        // When the context menu is triggered open, we check if we're part of the selection,
                        // and, if not, we update the selection to include only the item that was clicked.
                        if item_response.hovered() && item_response.secondary_clicked() {
                            show_context_menu(&item_collection);
                            ctx.command_sender()
                                .send_system(SystemCommand::set_selection(item_collection));
                        } else {
                            show_context_menu(ctx.selection());
                        }
                    } else {
                        show_context_menu(ctx.selection());
                    }
                }

                SelectionUpdateBehavior::OverrideSelection => {
                    show_context_menu(&item_collection);

                    if item_response.secondary_clicked() {
                        ctx.command_sender()
                            .send_system(SystemCommand::set_selection(item_collection));
                    }
                }

                SelectionUpdateBehavior::Ignore => {
                    show_context_menu(&item_collection);
                }
            }
        });
}

/// Returns the (statically-defined) list of action, grouped in sections.
///
/// Sections are group of actions that should be displayed together, with a separator displayed
/// between sections.
fn action_list(
    ctx: &ViewerContext<'_>,
) -> &'static Vec<Vec<Box<dyn ContextMenuAction + Sync + Send>>> {
    use egui_tiles::ContainerKind;

    static CONTEXT_MENU_ACTIONS: OnceLock<Vec<Vec<Box<dyn ContextMenuAction + Sync + Send>>>> =
        OnceLock::new();

    static_assertions::const_assert_eq!(ContainerKind::ALL.len(), 4);

    CONTEXT_MENU_ACTIONS.get_or_init(|| {
        vec![
            vec![
                Box::new(ShowAction),
                Box::new(HideAction),
                Box::new(RemoveAction),
                Box::new(CopyEntityPathToClipboard),
                Box::new(TrackEntity),
            ],
            vec![
                Box::new(actions::ScreenshotAction::CopyScreenshot),
                Box::new(actions::ScreenshotAction::SaveScreenshot),
            ],
            vec![
                Box::new(CollapseExpandAllAction::ExpandAll),
                Box::new(CollapseExpandAllAction::CollapseAll),
            ],
            vec![Box::new(CloneViewAction)],
            vec![
                Box::new(SubMenu {
                    label: "Add container".to_owned(),
                    actions: vec![
                        Box::new(AddContainerAction(ContainerKind::Tabs)),
                        Box::new(AddContainerAction(ContainerKind::Horizontal)),
                        Box::new(AddContainerAction(ContainerKind::Vertical)),
                        Box::new(AddContainerAction(ContainerKind::Grid)),
                    ],
                }),
                Box::new(SubMenu {
                    label: "Add view".to_owned(),
                    actions: ctx
                        .view_class_registry()
                        .iter_registry()
                        .map(|entry| {
                            Box::new(AddViewAction {
                                icon: entry.class.icon(),
                                id: entry.identifier,
                            })
                                as Box<dyn ContextMenuAction + Sync + Send>
                        })
                        .collect(),
                }),
            ],
            vec![Box::new(SubMenu {
                label: "Move to new container".to_owned(),
                actions: vec![
                    Box::new(MoveContentsToNewContainerAction(ContainerKind::Tabs)),
                    Box::new(MoveContentsToNewContainerAction(ContainerKind::Horizontal)),
                    Box::new(MoveContentsToNewContainerAction(ContainerKind::Vertical)),
                    Box::new(MoveContentsToNewContainerAction(ContainerKind::Grid)),
                ],
            })],
            vec![Box::new(AddEntitiesToNewViewAction)],
        ]
    })
}

/// Display every action that accepts the provided selection.
fn show_context_menu_for_selection(ctx: &ContextMenuContext<'_>, ui: &mut egui::Ui) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend); // TODO(#6801): remove mitigation against too narrow context menus

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
                ui.close();
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
    selection: &'a ItemCollection,
    clicked_item: &'a Item,
}

impl<'a> ContextMenuContext<'a> {
    /// Return the clicked item's parent container id and position within it.
    ///
    /// Valid only for views, containers, and data results. For data results, the parent and
    /// position of the enclosing view is considered.
    pub fn clicked_item_enclosing_container_id_and_position(&self) -> Option<(ContainerId, usize)> {
        let contents = match self.clicked_item {
            Item::View(view_id) => Contents::View(*view_id),
            Item::DataResult(data_result) => Contents::View(data_result.view_id),
            Item::Container(container_id) => Contents::Container(*container_id),
            _ => {
                return None;
            }
        };
        self.viewport_blueprint
            .find_parent_and_position_index(&contents)
    }

    /// Return the clicked item's parent container and position within it.
    ///
    /// Valid only for views, containers, and data results. For data results, the parent and
    /// position of the enclosing view is considered.
    pub fn clicked_item_enclosing_container_and_position(
        &self,
    ) -> Option<(&'a ContainerBlueprint, usize)> {
        let (container_id, pos) = self.clicked_item_enclosing_container_id_and_position()?;
        self.viewport_blueprint
            .container(&container_id)
            .map(|container| (container, pos))
    }

    pub fn egui_context(&self) -> &egui::Context {
        self.viewer_context.egui_ctx()
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
    /// Note: this is run from inside a [`egui::Response::context_menu()`] closure and must call
    /// [`Self::process_selection`] when triggered by the user.
    fn ui(&self, ctx: &ContextMenuContext<'_>, ui: &mut egui::Ui) -> egui::Response {
        let label = self.label(ctx);

        let response = if let Some(icon) = self.icon() {
            ui.add(icon.as_button_with_label(ui.tokens(), label))
        } else {
            ui.button(label)
        };
        if response.clicked() {
            self.process_selection(ctx);
        }
        response
    }

    fn icon(&self) -> Option<&'static re_ui::Icon> {
        None
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
                Item::AppId(app_id) => self.process_app_id(ctx, app_id),
                Item::TableId(table_id) => self.process_table_id(ctx, table_id),
                Item::DataSource(data_source) => self.process_data_source(ctx, data_source),
                Item::StoreId(store_id) => self.process_store_id(ctx, store_id),
                Item::ComponentPath(component_path) => {
                    self.process_component_path(ctx, component_path);
                }
                Item::View(view_id) => self.process_view(ctx, view_id),
                Item::InstancePath(instance_path) => self.process_instance_path(ctx, instance_path),
                Item::DataResult(data_result) => {
                    self.process_data_result(ctx, &data_result.view_id, &data_result.instance_path);
                }
                Item::Container(container_id) => self.process_container(ctx, container_id),
                Item::RedapServer(origin) => self.process_redap_server(ctx, origin),
                Item::RedapEntry(entry) => {
                    self.process_redap_entry(ctx, &entry.entry_id);
                }
            }
        }
    }

    fn process_app_id(&self, _ctx: &ContextMenuContext<'_>, _app_id: &re_log_types::ApplicationId) {
    }

    fn process_data_source(
        &self,
        _ctx: &ContextMenuContext<'_>,
        _data_source: &re_log_channel::LogSource,
    ) {
    }

    /// Process a single recording.
    fn process_store_id(&self, _ctx: &ContextMenuContext<'_>, _store_id: &re_log_types::StoreId) {}

    /// Process a table.
    fn process_table_id(&self, _ctx: &ContextMenuContext<'_>, _store_id: &TableId) {}

    /// Process a single container.
    fn process_container(&self, _ctx: &ContextMenuContext<'_>, _container_id: &ContainerId) {}

    /// Process a single redap server.
    fn process_redap_server(&self, _ctx: &ContextMenuContext<'_>, _origin: &re_uri::Origin) {}

    /// Process a single redap entry (dataset or table).
    fn process_redap_entry(
        &self,
        _ctx: &ContextMenuContext<'_>,
        _entry_id: &re_log_types::EntryId,
    ) {
    }

    /// Process a single view.
    fn process_view(&self, _ctx: &ContextMenuContext<'_>, _view_id: &ViewId) {}

    /// Process a single data result.
    fn process_data_result(
        &self,
        _ctx: &ContextMenuContext<'_>,
        _view_id: &ViewId,
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
