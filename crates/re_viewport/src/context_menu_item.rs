//TODO(ab): use list items to make those context menu nice to look at

use crate::{Contents, ViewportBlueprint};
use itertools::Itertools;
use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::{DataQueryBlueprint, SpaceViewBlueprint};
use re_viewer_context::{ContainerId, Item, SpaceViewClassIdentifier, ViewerContext};

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

//TODO(ab): this function must become much more complex and handle all cases of homogeneous and heterogeneous multi
//          selections
fn context_menu_items_for_item(
    ctx: &ViewerContext<'_>,
    item: &Item,
) -> Vec<Box<dyn ContextMenuItem>> {
    match item {
        Item::Container(container_id) => vec![
            ContentVisibilityToggle::item(Contents::Container(*container_id)),
            ContentRemove::item(Contents::Container(*container_id)),
            Separator::item(),
            SubMenu::item(
                "Add Container",
                [
                    AddContainer::item(*container_id, egui_tiles::ContainerKind::Tabs),
                    AddContainer::item(*container_id, egui_tiles::ContainerKind::Horizontal),
                    AddContainer::item(*container_id, egui_tiles::ContainerKind::Vertical),
                    AddContainer::item(*container_id, egui_tiles::ContainerKind::Grid),
                ],
            ),
            SubMenu::item(
                "Add Space View",
                ctx.space_view_class_registry
                    .iter_registry()
                    .sorted_by_key(|entry| entry.class.display_name())
                    .map(|entry| AddSpaceView::item(*container_id, entry.class.identifier())),
            ),
        ],
        Item::SpaceView(space_view_id) => vec![
            ContentVisibilityToggle::item(Contents::SpaceView(*space_view_id)),
            ContentRemove::item(Contents::SpaceView(*space_view_id)),
        ],
        Item::StoreId(_)
        | Item::ComponentPath(_)
        | Item::InstancePath(_, _)
        | Item::DataBlueprintGroup(_, _, _) => vec![],
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
        let actions = context_menu_items_for_item(ctx, item);
        for action in actions {
            let response = action.ui(ctx, viewport_blueprint, ui);
            if response.clicked() {
                ui.close_menu();
            }
        }
    });
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
    contents: Contents,
}

impl ContentVisibilityToggle {
    fn item(contents: Contents) -> Box<dyn ContextMenuItem> {
        Box::new(Self { contents })
    }
}

impl ContextMenuItem for ContentVisibilityToggle {
    fn label(&self, _ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) -> String {
        if viewport_blueprint.is_contents_visible(&self.contents) {
            "Hide".to_owned()
        } else {
            "Show".to_owned()
        }
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.set_content_visibility(
            ctx,
            &self.contents,
            !viewport_blueprint.is_contents_visible(&self.contents),
        );
    }
}

/// Remove a container or space view
struct ContentRemove {
    contents: Contents,
}

impl ContentRemove {
    fn item(contents: Contents) -> Box<dyn ContextMenuItem> {
        Box::new(Self { contents })
    }
}

impl ContextMenuItem for ContentRemove {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        "Remove".to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.mark_user_interaction(ctx);
        viewport_blueprint.remove_contents(self.contents);
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

    fn run(&self, _ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.add_container(self.container_kind, Some(self.target_container));
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
