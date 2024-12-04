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

use re_chunk::EntityPath;
use re_data_ui::item_ui::cursor_interact_with_selectable;
use re_entity_db::InstancePath;
use re_log_types::EntityPathPart;
use re_ui::{icons, list_item, DesignTokens, UiExt as _};
use re_viewer_context::{Contents, Item, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::ItemTitle;

const ICON_SCALE: f32 = 0.5; // Because we save all icons as 2x

/// We show this above each item section
pub fn item_heading_with_breadcrumbs(
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
                        ui.spacing_mut().item_spacing.x = 4.0;
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
    {
        // No background rectangles, even for hovered items
        let visuals = ui.visuals_mut();
        visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
    }

    ui.scope(|ui| {
        // Breadcrumbs
        {
            // Dimmer colors for breadcrumbs
            let visuals = ui.visuals_mut();
            visuals.widgets.inactive.fg_stroke.color = visuals.text_color();
        }

        match item {
            Item::AppId(_) | Item::DataSource(_) | Item::StoreId(_) => {
                // TODO(emilk): maybe some of these could have breadcrumbs
            }
            Item::InstancePath(instance_path) => {
                let InstancePath {
                    entity_path,
                    instance,
                } = instance_path;

                if instance.is_all() {
                    // Entity path
                    if let [ancestry @ .., _] = entity_path.as_slice() {
                        entity_path_breadcrumbs(ctx, ui, ancestry);
                    }
                } else {
                    // Instance path
                    entity_path_breadcrumbs(ctx, ui, entity_path.as_slice());
                }
            }
            Item::ComponentPath(component_path) => {
                entity_path_breadcrumbs(ctx, ui, component_path.entity_path.as_slice());
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

                // TODO(#4491): breadcrumbs for data results entity paths and projections
                // if let Some(view) = viewport.view(view_id) {
                //     let query_result = ctx.lookup_query_result(*view_id);
                //     let result_tree = &query_result.tree;
                //     let root_node = result_tree.root_node();
                // let origin =
                //     DataResultNodeOrPath::from_path_lookup(result_tree, &view.space_origin);
            }
        }
    });

    let ItemTitle {
        icon,
        label,
        label_style: _, // Intentionally ignored
        tooltip,
    } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

    let mut response = ui.add(
        egui::Button::image_and_text(icon.as_image().fit_to_original_size(ICON_SCALE), label)
            .image_tint_follows_text_color(true),
    );
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    cursor_interact_with_selectable(ctx, response, item.clone());
}

fn entity_path_breadcrumbs(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_parts: &[EntityPathPart],
) {
    // Match on everything plus last
    let button = match entity_parts {
        [ancestry @ .., last] => {
            // Recurse!
            entity_path_breadcrumbs(ctx, ui, ancestry);

            let first_char = last.unescaped_str().chars().next().unwrap_or('?');
            egui::Button::new(first_char.to_string()).image_tint_follows_text_color(true)
        }
        _ => {
            // Root
            egui::Button::image(icons::RECORDING.as_image().fit_to_original_size(ICON_SCALE))
                .image_tint_follows_text_color(true)
        }
    };

    let entity_path = EntityPath::new(entity_parts.to_vec());
    let response = ui.add(button).on_hover_text(entity_path.to_string());

    let item = Item::from(entity_path);
    cursor_interact_with_selectable(ctx, response, item);

    separator_icon_ui(ui, icons::BREADCRUMBS_SEPARATOR_ENTITY);
}

fn viewport_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    contents: Contents,
) {
    let item = Item::from(contents);

    if let Some(parent) = viewport.parent(&contents) {
        // Recurse!
        viewport_breadcrumbs(ctx, viewport, ui, parent.into());
    }

    let ItemTitle {
        icon,
        label: _,       // ignored: we just show the icon for breadcrumbs
        label_style: _, // no label
        tooltip,
    } = ItemTitle::from_item(ctx, viewport, ui.style(), &item);

    let mut response = ui.add(
        egui::Button::image(icon.as_image().fit_to_original_size(ICON_SCALE))
            .image_tint_follows_text_color(true),
    );
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    cursor_interact_with_selectable(ctx, response, item);

    separator_icon_ui(ui, icons::BREADCRUMBS_SEPARATOR_BLUEPRINT);
}

fn separator_icon_ui(ui: &mut egui::Ui, icon: re_ui::Icon) {
    ui.add(
        icon.as_image()
            .fit_to_original_size(ICON_SCALE)
            .tint(ui.visuals().text_color()),
    );
}
