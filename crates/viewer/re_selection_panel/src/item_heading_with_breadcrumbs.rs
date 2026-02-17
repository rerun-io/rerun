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
//!
//! The bread crumbs hierarchy should be identical to the hierarchy in the
//! either the blueprint tree panel, or the streams/time panel.

use egui::Color32;
use re_chunk::EntityPath;
use re_data_ui::item_ui::{cursor_interact_with_selectable, guess_instance_path_icon};
use re_entity_db::InstancePath;
use re_log_types::EntityPathPart;
use re_ui::{SyntaxHighlighting as _, UiExt as _, icons, list_item};
use re_viewer_context::{Contents, Item, ViewId, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::item_title::ItemTitle;

/// We show this above each item section
pub fn item_heading_with_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    re_tracing::profile_function!();

    let tokens = ui.tokens();

    ui.list_item()
        .with_height(tokens.title_bar_height())
        .interactive(false)
        .selected(true)
        .show_flat(
            ui,
            list_item::CustomContent::new(|ui, _| {
                ui.spacing_mut().item_spacing.x = 4.0;

                let tokens = ui.tokens();

                // First the C>R>U>M>B>S>
                // where icon color follows text color:
                {
                    let visuals = ui.visuals_mut();
                    visuals.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
                    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    visuals.widgets.active.weak_bg_fill = tokens.surface_on_primary_hovered;
                    visuals.widgets.hovered.weak_bg_fill = tokens.surface_on_primary_hovered;

                    visuals.widgets.noninteractive.fg_stroke.color = tokens.icon_color_on_primary;
                    visuals.widgets.inactive.fg_stroke.color = tokens.icon_color_on_primary;
                    visuals.widgets.active.fg_stroke.color = tokens.icon_color_on_primary_hovered;
                    visuals.widgets.hovered.fg_stroke.color = tokens.icon_color_on_primary_hovered;
                }

                item_bread_crumbs_ui(ctx, viewport, ui, item);
                {
                    let visuals = ui.visuals_mut();
                    visuals.widgets.noninteractive.fg_stroke.color = tokens.text_color_on_primary;
                    visuals.widgets.inactive.fg_stroke.color = tokens.text_color_on_primary;
                    visuals.widgets.active.fg_stroke.color = tokens.text_color_on_primary_hovered;
                    visuals.widgets.hovered.fg_stroke.color = tokens.text_color_on_primary_hovered;
                }

                // Then the full name of the main item:
                last_part_of_item_heading(ctx, viewport, ui, item);
            }),
        );
}

// Show the bread crumbs leading to (but not including) the final item.
fn item_bread_crumbs_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    match item {
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::RedapEntry(_)
        | Item::RedapServer(_)
        | Item::TableId(_) => {
            // These have no bread crumbs, at least not currently.
            // I guess one could argue that the `StoreId` should have the `AppId` as its ancestor?
        }
        Item::InstancePath(instance_path) => {
            let InstancePath {
                entity_path,
                instance,
            } = instance_path;

            if instance.is_all() {
                // Entity path. Exclude the last part from the breadcrumbs,
                // as we will show it in full later on.
                if let [all_but_last @ .., _] = entity_path.as_slice() {
                    entity_path_breadcrumbs(ctx, ui, None, &EntityPath::root(), all_but_last, true);
                }
            } else {
                // Instance path.
                // Show the full entity path, and save the `[instance_nr]` for later.
                entity_path_breadcrumbs(
                    ctx,
                    ui,
                    None,
                    &EntityPath::root(),
                    entity_path.as_slice(),
                    true,
                );
            }
        }
        Item::ComponentPath(component_path) => {
            entity_path_breadcrumbs(
                ctx,
                ui,
                None,
                &EntityPath::root(),
                component_path.entity_path.as_slice(),
                true,
            );
        }
        Item::Container(container_id) => {
            if let Some(parent) = viewport.parent(&Contents::Container(*container_id)) {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::Container(parent));
            }
        }
        Item::View(view_id) => {
            if let Some(parent) = viewport.parent(&Contents::View(*view_id)) {
                viewport_breadcrumbs(ctx, viewport, ui, Contents::Container(parent));
            }
        }
        Item::DataResult(data_result) => {
            viewport_breadcrumbs(ctx, viewport, ui, Contents::View(data_result.view_id));

            let InstancePath {
                entity_path,
                instance,
            } = &data_result.instance_path;

            if let Some(view) = viewport.view(&data_result.view_id) {
                let common_ancestor = data_result
                    .instance_path
                    .entity_path
                    .common_ancestor(&view.space_origin);

                let relative = &entity_path.as_slice()[common_ancestor.len()..];

                let is_projection = !entity_path.starts_with(&view.space_origin);
                // TODO(#10649): the projection breadcrumbs are wrong for nuscenes (but correct for arkit!),
                // at least if we consider the blueprint tree panel as "correct".
                // I fear we need to use the undocumented `DataResultNodeOrPath` and friends to match the
                // hierarchy of the blueprint tree panel.

                if instance.is_all() {
                    // Entity path. Exclude the last part from the breadcrumbs,
                    // as we will show it in full later on.
                    if let [all_but_last @ .., _] = relative {
                        entity_path_breadcrumbs(
                            ctx,
                            ui,
                            Some(data_result.view_id),
                            &common_ancestor,
                            all_but_last,
                            !is_projection,
                        );
                    }
                } else {
                    // Instance path.
                    // Show the full entity path, and save the `[instance_nr]` for later.
                    entity_path_breadcrumbs(
                        ctx,
                        ui,
                        Some(data_result.view_id),
                        &common_ancestor,
                        relative,
                        !is_projection,
                    );
                }
            }
        }
    }
}

// Show the actual item, after all the bread crumbs:
fn last_part_of_item_heading(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    let ItemTitle {
        icon,
        label,
        label_style: _, // Intentionally ignored
        tooltip,
    } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

    let with_icon = match item {
        Item::AppId { .. }
        | Item::DataSource { .. }
        | Item::Container { .. }
        | Item::View { .. }
        | Item::TableId { .. }
        | Item::StoreId { .. }
        | Item::RedapEntry(_)
        | Item::RedapServer(_) => true,

        Item::InstancePath { .. } | Item::DataResult { .. } | Item::ComponentPath { .. } => false,
    };

    let mut response = if with_icon {
        ui.selectable_label_with_icon(
            icon,
            label,
            ctx.is_selected_or_loading(item),
            re_ui::LabelStyle::Normal,
        )
    } else {
        ui.add(egui::Button::new(label).truncate())
    };
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    cursor_interact_with_selectable(ctx, response, item.clone());
}

/// The breadcrumbs of containers and views in the viewport.
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
    } = ItemTitle::from_contents(ctx, viewport, &contents);

    let mut response = ui.add(icon.as_button());
    if let Some(tooltip) = tooltip {
        response = response.on_hover_text(tooltip);
    }
    cursor_interact_with_selectable(ctx, response, item);

    separator_icon_ui(ui);
}

pub fn separator_icon_ui(ui: &mut egui::Ui) {
    ui.add(
        icons::BREADCRUMBS_SEPARATOR
            .as_image()
            .tint(ui.tokens().icon_color_on_primary),
    );
}

/// The breadcrumbs of an entity path,
/// that may or may not be part of a view.
fn entity_path_breadcrumbs(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    // If we are in a view
    view_id: Option<ViewId>,
    // Everything is relative to this
    origin: &EntityPath,
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

    let full_entity_path = origin.join(&EntityPath::new(entity_parts.to_vec()));

    let button = if let Some(last) = full_entity_path.last() {
        let first_char = last.unescaped_str().chars().next().unwrap_or('?');
        egui::Button::new(first_char.to_string())
    } else {
        // Root
        let icon = if view_id.is_some() {
            // Inside a view, we show the root with an icon
            // that matches the one in the blueprint tree panel.
            guess_instance_path_icon(ctx, &InstancePath::from(full_entity_path.clone()))
        } else {
            // For a streams hierarchy, we show the root using a different icon,
            // just to make it clear that this is a different kind of hierarchy.
            &icons::RECORDING // streams hierarchy
        };
        icon.as_button()
    };

    let response = ui.add(button);
    let response = response.on_hover_ui(|ui| {
        ui.label(full_entity_path.syntax_highlighted(ui.style()));
    });

    let item = if let Some(view_id) = view_id {
        Item::DataResult(
            re_viewer_context::DataResultInteractionAddress::from_entity_path(
                view_id,
                full_entity_path,
            ),
        )
    } else {
        Item::from(full_entity_path)
    };
    cursor_interact_with_selectable(ctx, response, item);

    separator_icon_ui(ui);
}
