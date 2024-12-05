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
use re_ui::{icons, list_item, DesignTokens, SyntaxHighlighting, UiExt as _};
use re_viewer_context::{Contents, Item, SpaceViewId, ViewerContext};
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
    re_tracing::profile_function!();

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
                        entity_path_breadcrumbs(ctx, ui, None, None, ancestry, true);
                    }
                } else {
                    // Instance path
                    entity_path_breadcrumbs(ctx, ui, None, None, entity_path.as_slice(), true);
                }
            }
            Item::ComponentPath(component_path) => {
                entity_path_breadcrumbs(
                    ctx,
                    ui,
                    None,
                    None,
                    component_path.entity_path.as_slice(),
                    true,
                );
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
            Item::DataResult(view_id, instance_path) => {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::SpaceView(*view_id));

                let InstancePath {
                    entity_path,
                    instance,
                } = instance_path;

                if let Some(view) = viewport.view(view_id) {
                    if entity_path.starts_with(&view.space_origin) {
                        let relative = &entity_path.as_slice()[view.space_origin.len()..];

                        if instance.is_all() {
                            // we will show last part in full later
                            if let [all_but_last @ .., _] = relative {
                                entity_path_breadcrumbs(
                                    ctx,
                                    ui,
                                    Some(*view_id),
                                    Some(&view.space_origin),
                                    all_but_last,
                                    true,
                                );
                            }
                        } else {
                            // we show the instance number later
                            entity_path_breadcrumbs(
                                ctx,
                                ui,
                                Some(*view_id),
                                Some(&view.space_origin),
                                relative,
                                true,
                            );
                        }
                    } else {
                        // Projections
                        let common_ancestor = instance_path
                            .entity_path
                            .common_ancestor(&view.space_origin);
                        let relative = &entity_path.as_slice()[common_ancestor.len()..];

                        if instance.is_all() {
                            // we will show last part in full later
                            if let [all_but_last @ .., _] = relative {
                                entity_path_breadcrumbs(
                                    ctx,
                                    ui,
                                    Some(*view_id),
                                    Some(&common_ancestor),
                                    all_but_last,
                                    false,
                                );
                            }
                        } else {
                            // we show the instance number later
                            entity_path_breadcrumbs(
                                ctx,
                                ui,
                                Some(*view_id),
                                Some(&common_ancestor),
                                relative,
                                false,
                            );
                        }
                    }
                }
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
    // If we are in a view
    view_id: Option<SpaceViewId>,
    // Everything is relative to this
    origin: Option<&EntityPath>,
    // Show crumbs for all of these
    entity_parts: &[EntityPathPart],
    include_root: bool,
) {
    if let [ancestry @ .., _] = entity_parts {
        // Recurse!

        if !ancestry.is_empty() || include_root {
            entity_path_breadcrumbs(ctx, ui, view_id, origin, ancestry, include_root);
        }
    }

    let full_entity_path = if let Some(origin) = origin {
        origin.join(&EntityPath::new(entity_parts.to_vec()))
    } else {
        EntityPath::new(entity_parts.to_vec())
    };

    let button = if let Some(last) = full_entity_path.last() {
        let first_char = last.unescaped_str().chars().next().unwrap_or('?');
        egui::Button::new(first_char.to_string())
    } else {
        // Root
        // TODO: different icon, at least if we are in a space view
        egui::Button::image(icons::RECORDING.as_image().fit_to_original_size(ICON_SCALE))
            .image_tint_follows_text_color(true)
    };

    let response = ui.add(button);
    let response = response.on_hover_ui(|ui| {
        ui.label(full_entity_path.syntax_highlighted(ui.style()));
    });

    let item = if let Some(view_id) = view_id {
        Item::DataResult(view_id, full_entity_path.into())
    } else {
        Item::from(full_entity_path)
    };
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
