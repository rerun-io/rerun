use egui::WidgetText;
use re_data_ui::item_ui::{cursor_interact_with_selectable, guess_instance_path_icon};
use re_log_types::ComponentPath;
use re_ui::{Icon, SyntaxHighlighting as _, UiExt as _, icons, list_item};
use re_viewer_context::{Item, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::item_title::{ItemTitle, is_component_static};

/// The same size that [`re_ui::UiExt::paint_collapsing_triangle`] uses,
/// which keeps the visual language consistent.
const CHEVRON_ICON_SIZE: egui::Vec2 = egui::Vec2::new(8.0, 8.0);

fn separator_icon_ui(ui: &mut egui::Ui, tint: egui::Color32) {
    ui.add(
        icons::CHEVRON
            .as_image()
            .max_size(CHEVRON_ICON_SIZE)
            .tint(tint),
    );
}

/// Just the title of the item; for when multiple items are selected
pub fn item_title_list_item(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    let response = ui
        .list_item()
        .with_height(re_ui::DesignTokens::list_item_height())
        .interactive(true)
        .show_flat(
            ui,
            list_item::CustomContent::new(|ui, context| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.style_mut().interaction.selectable_labels = false;

                // The icons are white by default, so they need a tint that matches
                // the list item background (light, dark, hovered or selected).
                let icon_tint = context.visuals.icon_tint();
                item_heading_no_breadcrumbs(ctx, viewport, ui, item, icon_tint);
            }),
        );
    cursor_interact_with_selectable(&ctx.app_ctx, response, item.clone());
}

/// Fully descriptive heading for an item, without any breadcrumbs.
fn item_heading_no_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
    icon_tint: egui::Color32,
) {
    match item {
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::TableId(_)
        | Item::Container(_)
        | Item::View(_)
        | Item::RedapEntry { .. }
        | Item::RedapServer(_) => {
            let ItemTitle {
                icon,
                label,
                label_style: _, // no label
                tooltip: _,
            } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

            icon_and_title(ui, icon, label, icon_tint);
        }
        Item::InstancePath(instance_path) => {
            icon_and_title(
                ui,
                guess_instance_path_icon(ctx, instance_path),
                instance_path.syntax_highlighted(ui.style()),
                icon_tint,
            );
        }
        Item::ComponentPath(component_path) => {
            let is_component_static = is_component_static(ctx, component_path);

            // Break up into entity path and component descriptor:
            let ComponentPath {
                entity_path,
                component,
            } = component_path;

            item_heading_no_breadcrumbs(
                ctx,
                viewport,
                ui,
                &Item::from(entity_path.clone()),
                icon_tint,
            );

            separator_icon_ui(ui, icon_tint);

            let component_icon = if is_component_static {
                &icons::COMPONENT_STATIC
            } else {
                &icons::COMPONENT_TEMPORAL
            };
            icon_and_title(
                ui,
                component_icon,
                component.syntax_highlighted(ui.style()),
                icon_tint,
            );
        }
        Item::DataResult(data_result) => {
            // Break up into view and instance path:
            item_heading_no_breadcrumbs(
                ctx,
                viewport,
                ui,
                &Item::View(data_result.view_id),
                icon_tint,
            );
            separator_icon_ui(ui, icon_tint);
            item_heading_no_breadcrumbs(
                ctx,
                viewport,
                ui,
                &Item::InstancePath(data_result.instance_path.clone()),
                icon_tint,
            );
        }
    }
}

fn icon_and_title(
    ui: &mut egui::Ui,
    icon: &Icon,
    title: impl Into<WidgetText>,
    icon_tint: egui::Color32,
) {
    ui.add(
        icon.as_image()
            .fit_to_exact_size(ui.tokens().small_icon_size)
            .tint(icon_tint),
    );
    ui.label(title);
}
