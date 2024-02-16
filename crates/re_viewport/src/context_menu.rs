//TODO(ab): use list items to make those context menu nice to look at

use crate::{Contents, ViewportBlueprint};
use itertools::Itertools;
use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::{DataQueryBlueprint, SpaceViewBlueprint};
use re_viewer_context::{ContainerId, Item, Selection, SpaceViewClassIdentifier, ViewerContext};

/// Trait for things that can populate a context menu
trait ContextMenuItem {
    //TODO(ab): should probably return `egui::WidgetText` instead
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        String::new()
    }

    fn run(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) {}

    /// run from inside of [`egui::Response.context_menu()`]
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        let label = self.label(ctx, viewport_blueprint);
        let response = ui.button(label);
        if response.clicked() {
            self.run(ctx, viewport_blueprint);
        }
        response
    }
}

fn context_menu_items_for_selection_summary(
    ctx: &ViewerContext<'_>,
    viewport_blueprint: &ViewportBlueprint,
    selection_summary: SelectionSummary,
) -> Vec<Box<dyn ContextMenuItem>> {
    match selection_summary {
        SelectionSummary::SingleContainerItem(container_id) => {
            let mut items = vec![];

            // only show/hide and remove if it's not the root container
            if Some(container_id) != viewport_blueprint.root_container {
                let contents = vec![Contents::Container(container_id)];
                items.extend([
                    ContentVisibilityToggle::item(viewport_blueprint, contents.clone()),
                    ContentRemove::item(contents),
                    Separator::item(),
                ]);
            }

            items.extend([
                SubMenu::item(
                    "Add Container",
                    [
                        AddContainer::item(container_id, egui_tiles::ContainerKind::Tabs),
                        AddContainer::item(container_id, egui_tiles::ContainerKind::Horizontal),
                        AddContainer::item(container_id, egui_tiles::ContainerKind::Vertical),
                        AddContainer::item(container_id, egui_tiles::ContainerKind::Grid),
                    ],
                ),
                SubMenu::item(
                    "Add Space View",
                    ctx.space_view_class_registry
                        .iter_registry()
                        .sorted_by_key(|entry| entry.class.display_name())
                        .map(|entry| AddSpaceView::item(container_id, entry.class.identifier())),
                ),
            ]);

            items
        }
        SelectionSummary::ContentsItems(contents) => {
            // exclude the root container from the list of contents, as it cannot be shown/hidden
            // nor removed
            let contents: Vec<_> = contents
                .into_iter()
                .filter(|c| Some(*c) != viewport_blueprint.root_container.map(Contents::Container))
                .collect();

            if contents.is_empty() {
                vec![]
            } else {
                vec![
                    ContentVisibilityToggle::item(viewport_blueprint, contents.clone()),
                    ContentRemove::item(contents),
                ]
            }
        }
        SelectionSummary::Heterogeneous | SelectionSummary::Empty => vec![],
    }
}

/// Display a context menu for the provided [`Item`]
pub fn context_menu_ui_for_item(
    ctx: &ViewerContext<'_>,
    viewport_blueprint: &ViewportBlueprint,
    item: &Item,
    item_response: &egui::Response,
) {
    item_response.context_menu(|ui| {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.close_menu();
            return;
        }

        // handle selection
        let selection_summary = if !ctx.selection().contains_item(item) {
            // When the context menu is triggered open, we check if we're part of the selection,
            // and, if not, we update the selection to include only the item that was clicked.
            if item_response.hovered() && item_response.secondary_clicked() {
                ctx.selection_state()
                    .set_selection(std::iter::once(item.clone()));

                summarize_selection(&Selection::from(item.clone()))
            } else {
                summarize_selection(ctx.selection())
            }
        } else {
            summarize_selection(ctx.selection())
        };

        let actions =
            context_menu_items_for_selection_summary(ctx, viewport_blueprint, selection_summary);

        if actions.is_empty() {
            ui.label(
                egui::RichText::from("No action available for the current selection").italics(),
            );
        }

        for action in actions {
            let response = action.ui(ctx, viewport_blueprint, ui);
            if response.clicked() {
                ui.close_menu();
            }
        }
    });
}

// ================================================================================================
// Selection summary
// ================================================================================================

// TODO(ab): this summary is somewhat ad hoc to the context menu needs. Could it be generalised and
// moved to the Selection itself?
#[derive(Debug, Clone)]
pub enum SelectionSummary {
    SingleContainerItem(ContainerId),
    ContentsItems(Vec<Contents>),
    Heterogeneous,
    Empty,
}

fn summarize_selection(selection: &Selection) -> SelectionSummary {
    if selection.is_empty() {
        return SelectionSummary::Empty;
    }

    if selection.len() == 1 {
        if let Some(Item::Container(container_id)) = selection.first_item() {
            return SelectionSummary::SingleContainerItem(*container_id);
        }
    }

    // test if the selection contains only contents
    let only_space_view_or_container_only = selection
        .iter()
        .all(|(item, _)| matches!(item, Item::Container(_) | Item::SpaceView(_)));

    if only_space_view_or_container_only {
        let contents = selection
            .iter()
            .filter_map(|(item, _)| match item {
                Item::Container(container_id) => Some(Contents::Container(*container_id)),
                Item::SpaceView(space_view_id) => Some(Contents::SpaceView(*space_view_id)),
                _ => None,
            })
            .collect();
        return SelectionSummary::ContentsItems(contents);
    }

    SelectionSummary::Heterogeneous
}

// ================================================================================================
// Utility items
// ================================================================================================

/// Group items into a sub-menu
struct SubMenu {
    label: String,
    actions: Vec<Box<dyn ContextMenuItem>>,
}

impl SubMenu {
    fn item(
        label: &str,
        actions: impl IntoIterator<Item = Box<dyn ContextMenuItem>>,
    ) -> Box<dyn ContextMenuItem> {
        let actions = actions.into_iter().collect();
        Box::new(Self {
            label: label.to_owned(),
            actions,
        })
    }
}

impl ContextMenuItem for SubMenu {
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        ui.menu_button(&self.label, |ui| {
            for action in &self.actions {
                let response = action.ui(ctx, viewport_blueprint, ui);
                if response.clicked() {
                    ui.close_menu();
                }
            }
        })
        .response
    }
}

/// Add a separator to the context menu
struct Separator;

impl Separator {
    fn item() -> Box<dyn ContextMenuItem> {
        Box::new(Self)
    }
}

impl ContextMenuItem for Separator {
    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        ui.separator()
    }
}

// ================================================================================================
// Space View/Container edit items
// ================================================================================================

/// Control the visibility of a container or space view
struct ContentVisibilityToggle {
    contents: Vec<Contents>,
    set_visible: bool,
}

impl ContentVisibilityToggle {
    fn item(
        viewport_blueprint: &ViewportBlueprint,
        contents: Vec<Contents>,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            set_visible: !contents
                .iter()
                .all(|item| viewport_blueprint.is_contents_visible(item)),
            contents,
        })
    }
}

impl ContextMenuItem for ContentVisibilityToggle {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        if self.set_visible {
            "Show".to_owned()
        } else {
            "Hide".to_owned()
        }
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        for content in &self.contents {
            viewport_blueprint.set_content_visibility(ctx, content, self.set_visible);
        }
    }
}

/// Remove a container or space view
struct ContentRemove {
    contents: Vec<Contents>,
}

impl ContentRemove {
    fn item(contents: Vec<Contents>) -> Box<dyn ContextMenuItem> {
        Box::new(Self { contents })
    }
}

impl ContextMenuItem for ContentRemove {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        "Remove".to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        for content in &self.contents {
            viewport_blueprint.mark_user_interaction(ctx);
            viewport_blueprint.remove_contents(*content);
        }
    }
}

// ================================================================================================
// Container items
// ================================================================================================

/// Add a container of a specific type
struct AddContainer {
    target_container: ContainerId,
    container_kind: egui_tiles::ContainerKind,
}

impl AddContainer {
    fn item(
        target_container: ContainerId,
        container_kind: egui_tiles::ContainerKind,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            target_container,
            container_kind,
        })
    }
}

impl ContextMenuItem for AddContainer {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        format!("{:?}", self.container_kind)
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.add_container(self.container_kind, Some(self.target_container));
        viewport_blueprint.mark_user_interaction(ctx);
    }
}

// ---

/// Add a space view of the specific class
struct AddSpaceView {
    target_container: ContainerId,
    space_view_class: SpaceViewClassIdentifier,
}

impl AddSpaceView {
    fn item(
        target_container: ContainerId,
        space_view_class: SpaceViewClassIdentifier,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            target_container,
            space_view_class,
        })
    }
}

impl ContextMenuItem for AddSpaceView {
    fn label(&self, ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        ctx.space_view_class_registry
            .get_class_or_log_error(&self.space_view_class)
            .display_name()
            .to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        let space_view = SpaceViewBlueprint::new(
            self.space_view_class,
            &EntityPath::root(),
            DataQueryBlueprint::new(self.space_view_class, EntityPathFilter::default()),
        );

        viewport_blueprint.add_space_views(
            std::iter::once(space_view),
            ctx,
            Some(self.target_container),
        );
        viewport_blueprint.mark_user_interaction(ctx);
    }
}
