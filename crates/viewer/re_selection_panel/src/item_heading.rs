//! The heading of each item in the selection panel.
//!
//! It consists of a blue background (selected background color),
//! and within it there are "bread-crumbs" that show the hierarchy of the item.
//!
//! A > B > C > D > item
//!
//! Each bread-crumb is just an icon or a letter.
//! The item is an icon and a name.
//! Each bread-crumb is clickable, as is the last item.

use re_data_ui::item_ui::cursor_interact_with_selectable;
use re_ui::{icons, list_item, DesignTokens, UiExt as _};
use re_viewer_context::{Contents, Item, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::ItemTitle;

const ICON_SCALE: f32 = 0.5; // Because we save all icons as 2x

/// We show this above each item section
pub fn item_heading(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    ui.list_item()
        .with_height(DesignTokens::title_bar_height())
        .interactive(false)
        .selected(true)
        .show_flat(
            ui,
            list_item::CustomContent::new(|ui, context| {
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(context.rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        item_heading_contents(ctx, viewport, ui, item);
                    },
                );
            }),
        );
}

fn item_heading_contents(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    match item {
        Item::AppId(_) | Item::DataSource(_) | Item::StoreId(_) => {
            // TODO(emilk): maybe some of these could have
        }
        Item::InstancePath(_) => {
            // TODO: bread-crumbs of the entity path
        }
        Item::ComponentPath(component_path) => {
            // TODO: bread-crumbs of the entity path
        }
        Item::Container(container_id) => {
            if let Some(parent) = viewport.parent(&Contents::Container(*container_id)) {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::Container(parent));
            }
        }
        Item::SpaceView(view_id) => {
            if let Some(parent) = viewport.parent(&Contents::SpaceView(*view_id)) {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::Container(parent));
            }
        }
        Item::DataResult(view_id, _) => {
            viewport_breadcrumbs(ctx, viewport, ui, Contents::SpaceView(*view_id));
        }
    }

    let ItemTitle {
        name,
        tooltip,
        icon,
        label_style,
    } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

    let mut response = ui.add(egui::Button::image_and_text(
        icon.as_image().fit_to_original_size(ICON_SCALE),
        name,
    ));
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    cursor_interact_with_selectable(ctx, response, item.clone());
}

fn viewport_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    contents: Contents,
) {
    let item = Item::from(contents);

    if let Some(parent) = viewport.parent(&contents) {
        viewport_breadcrumbs(ctx, viewport, ui, parent.into());
    }

    let ItemTitle {
        name: _, // ignored: we just show the icon for breadcrumbs
        tooltip,
        icon,
        label_style: _, // no label
    } = ItemTitle::from_item(ctx, viewport, ui.style(), &item);

    let mut response = ui.add(egui::Button::image(
        icon.as_image().fit_to_original_size(ICON_SCALE),
    ));
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    cursor_interact_with_selectable(ctx, response, item);

    ui.add(
        icons::BREADCRUMBS_SEPARATOR
            .as_image()
            .fit_to_original_size(ICON_SCALE),
    );
}
