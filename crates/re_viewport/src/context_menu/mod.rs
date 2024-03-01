use std::rc::Rc;

use itertools::Itertools;

use re_viewer_context::{ContainerId, Item, Selection, SpaceViewId, ViewerContext};

use crate::{Contents, ViewportBlueprint};

mod container_and_space_view_actions;
//mod space_view_data;
mod utils;

use container_and_space_view_actions::{
    AddContainer, AddSpaceView, CloneSpaceViewItem, ContentRemove, ContentVisibilityToggle,
    MoveContentsToNewContainer,
};
//use space_view_data::SpaceViewData;
use utils::{Separator, SubMenu};

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

        // handle selection
        let selection_summary = match selection_update_behavior {
            SelectionUpdateBehavior::UseSelection => {
                if !ctx.selection().contains_item(item) {
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
                }
            }

            SelectionUpdateBehavior::OverrideSelection => {
                if item_response.secondary_clicked() {
                    ctx.selection_state()
                        .set_selection(std::iter::once(item.clone()));
                }

                summarize_selection(&Selection::from(item.clone()))
            }

            SelectionUpdateBehavior::Ignore => summarize_selection(&Selection::from(item.clone())),
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

// ---

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
enum SelectionSummary {
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

    // check if we have only space views or containers
    let only_space_view_or_container: Option<Vec<_>> = selection
        .iter()
        .map(|(item, _)| match item {
            Item::Container(container_id) => Some(Contents::Container(*container_id)),
            Item::SpaceView(space_view_id) => Some(Contents::SpaceView(*space_view_id)),
            _ => None,
        })
        .collect();
    if let Some(contents) = only_space_view_or_container {
        return SelectionSummary::ContentsItems(contents);
    }

    SelectionSummary::Heterogeneous
}
