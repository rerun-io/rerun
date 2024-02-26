use std::rc::Rc;

use itertools::Itertools;

use re_log_types::{EntityPath, EntityPathFilter};
use re_space_view::{DataQueryBlueprint, SpaceViewBlueprint};
use re_viewer_context::{
    ContainerId, Item, Selection, SpaceViewClassIdentifier, SpaceViewId, ViewerContext,
};

use crate::{Contents, ViewportBlueprint};

/// Trait for things that can populate a context menu
trait ContextMenuItem {
    // TODO(ab): return a `ListItem` to make those context menu nice to look at. This requires
    // changes to the context menu UI code to support full-span highlighting.
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
    item: &Item,
    selection_summary: SelectionSummary,
) -> Vec<Box<dyn ContextMenuItem>> {
    match selection_summary {
        SelectionSummary::SingleContainerItem(container_id) => {
            // We want all the actions available for collections of contents…
            let mut items = context_menu_items_for_selection_summary(
                ctx,
                viewport_blueprint,
                item,
                SelectionSummary::ContentsItems(vec![Contents::Container(container_id)]),
            );

            if !items.is_empty() {
                items.push(Separator::item());
            }

            // …plus some more that apply to single container only.
            items.extend([
                SubMenu::item(
                    "Add Container",
                    possible_child_container_kind(viewport_blueprint, container_id)
                        .map(|kind| AddContainer::item(container_id, kind)),
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
        SelectionSummary::SingleSpaceView(space_view_id) => {
            // We want all the actions available for collections of contents…
            let mut items = context_menu_items_for_selection_summary(
                ctx,
                viewport_blueprint,
                item,
                SelectionSummary::ContentsItems(vec![Contents::SpaceView(space_view_id)]),
            );

            items.push(CloneSpaceViewItem::item(space_view_id));

            items
        }
        SelectionSummary::ContentsItems(contents) => {
            // exclude the root container from the list of contents, as it cannot be shown/hidden
            // nor removed
            let contents: Rc<Vec<_>> = Rc::new(
                contents
                    .into_iter()
                    .filter(|c| {
                        Some(*c) != viewport_blueprint.root_container.map(Contents::Container)
                    })
                    .collect(),
            );

            if contents.is_empty() {
                vec![]
            } else if let Some(root_container_id) = viewport_blueprint.root_container {
                // The new container should be created in place of the right-clicked content, so we
                // look for its parent and position, and fall back to the root container.
                let clicked_content = match item {
                    Item::Container(container_id) => Some(Contents::Container(*container_id)),
                    Item::SpaceView(space_view_id) => Some(Contents::SpaceView(*space_view_id)),
                    _ => None,
                };
                let (target_container_id, target_position) = clicked_content
                    .and_then(|c| viewport_blueprint.find_parent_and_position_index(&c))
                    .unwrap_or((root_container_id, 0));

                vec![
                    ContentVisibilityToggle::item(viewport_blueprint, contents.clone()),
                    ContentRemove::item(contents.clone()),
                    Separator::item(),
                    SubMenu::item(
                        "Move to new container",
                        possible_child_container_kind(viewport_blueprint, target_container_id).map(
                            |kind| {
                                MoveContentsToNewContainer::item(
                                    target_container_id,
                                    target_position,
                                    kind,
                                    contents.clone(),
                                )
                            },
                        ),
                    ),
                ]
            } else {
                vec![]
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

        let actions = context_menu_items_for_selection_summary(
            ctx,
            viewport_blueprint,
            item,
            selection_summary,
        );

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

/// Helper that returns the allowable containers
fn possible_child_container_kind(
    viewport_blueprint: &ViewportBlueprint,
    container_id: ContainerId,
) -> impl Iterator<Item = egui_tiles::ContainerKind> + 'static {
    let container_kind = viewport_blueprint
        .container(&container_id)
        .map(|c| c.container_kind);

    static ALL_CONTAINERS: &[egui_tiles::ContainerKind] = &[
        egui_tiles::ContainerKind::Tabs,
        egui_tiles::ContainerKind::Horizontal,
        egui_tiles::ContainerKind::Vertical,
        egui_tiles::ContainerKind::Grid,
    ];

    ALL_CONTAINERS
        .iter()
        .copied()
        .filter(move |kind| match kind {
            egui_tiles::ContainerKind::Horizontal | egui_tiles::ContainerKind::Vertical => {
                container_kind != Some(*kind)
            }
            _ => true,
        })
}

// ================================================================================================
// Selection summary
// ================================================================================================

// TODO(ab): this summary is somewhat ad hoc to the context menu needs. Could it be generalised and
// moved to the Selection itself?
#[derive(Debug, Clone)]
pub enum SelectionSummary {
    SingleContainerItem(ContainerId),
    SingleSpaceView(SpaceViewId),
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
        } else if let Some(Item::SpaceView(space_view_id)) = selection.first_item() {
            return SelectionSummary::SingleSpaceView(*space_view_id);
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
    contents: Rc<Vec<Contents>>,
    set_visible: bool,
}

impl ContentVisibilityToggle {
    fn item(
        viewport_blueprint: &ViewportBlueprint,
        contents: Rc<Vec<Contents>>,
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
        for content in &*self.contents {
            viewport_blueprint.set_content_visibility(ctx, content, self.set_visible);
        }
    }
}

/// Remove a container or space view
struct ContentRemove {
    contents: Rc<Vec<Contents>>,
}

impl ContentRemove {
    fn item(contents: Rc<Vec<Contents>>) -> Box<dyn ContextMenuItem> {
        Box::new(Self { contents })
    }
}

impl ContextMenuItem for ContentRemove {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        "Remove".to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        for content in &*self.contents {
            viewport_blueprint.mark_user_interaction(ctx);
            viewport_blueprint.remove_contents(*content);
        }
    }
}

// ================================================================================================
// Space view items
// ================================================================================================

/// Clone a space view
struct CloneSpaceViewItem {
    space_view_id: SpaceViewId,
}

impl CloneSpaceViewItem {
    fn item(space_view_id: SpaceViewId) -> Box<dyn ContextMenuItem> {
        Box::new(Self { space_view_id })
    }
}

impl ContextMenuItem for CloneSpaceViewItem {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        "Clone".to_owned()
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        if let Some(new_space_view_id) =
            viewport_blueprint.duplicate_space_view(&self.space_view_id, ctx)
        {
            ctx.selection_state()
                .set_selection(Item::SpaceView(new_space_view_id));
            viewport_blueprint.mark_user_interaction(ctx);
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
            None,
        );
        viewport_blueprint.mark_user_interaction(ctx);
    }
}

// ---

/// Move the selected contents to a newly created container of the given kind
struct MoveContentsToNewContainer {
    parent_container: ContainerId,
    position_in_parent: usize,
    container_kind: egui_tiles::ContainerKind,
    contents: Rc<Vec<Contents>>,
}

impl MoveContentsToNewContainer {
    fn item(
        parent_container: ContainerId,
        position_in_parent: usize,
        container_kind: egui_tiles::ContainerKind,
        contents: Rc<Vec<Contents>>,
    ) -> Box<dyn ContextMenuItem> {
        Box::new(Self {
            parent_container,
            position_in_parent,
            container_kind,
            contents,
        })
    }
}

impl ContextMenuItem for MoveContentsToNewContainer {
    fn label(&self, _ctx: &ViewerContext<'_>, _viewport_blueprint: &ViewportBlueprint) -> String {
        format!("{:?}", self.container_kind)
    }

    fn run(&self, ctx: &ViewerContext<'_>, viewport_blueprint: &ViewportBlueprint) {
        viewport_blueprint.move_contents_to_new_container(
            (*self.contents).clone(),
            self.container_kind,
            self.parent_container,
            self.position_in_parent,
        );

        viewport_blueprint.mark_user_interaction(ctx);
    }
}
