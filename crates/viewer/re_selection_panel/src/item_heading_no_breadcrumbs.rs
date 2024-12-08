use re_data_ui::item_ui::guess_instance_path_icon;
use re_ui::{icons, SyntaxHighlighting};
use re_viewer_context::{Item, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::{
    item_heading_with_breadcrumbs::separator_icon_ui,
    item_title::{is_component_static, ItemTitle},
};

/// Fully descriptive heading for an item, without any breadcrumbs.
pub fn item_heading_no_breadcrumbs(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &mut egui::Ui,
    item: &Item,
) {
    match item {
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::Container(_)
        | Item::SpaceView(_) => {
            let ItemTitle {
                icon,
                label,
                label_style: _, // no label
                tooltip,
            } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

            let response = ui.add(egui::Button::image_and_text(icon.as_image(), label));
            if let Some(tooltip) = tooltip {
                response.on_hover_text(tooltip);
            }
        }
        Item::InstancePath(instance_path) => {
            ui.add(egui::Button::image_and_text(
                guess_instance_path_icon(ctx, instance_path),
                instance_path.syntax_highlighted(ui.style()),
            ));
        }
        Item::ComponentPath(component_path) => {
            let is_static = is_component_static(ctx, component_path);
            ui.add(egui::Button::image_and_text(
                if is_static {
                    &icons::COMPONENT_STATIC
                } else {
                    &icons::COMPONENT_TEMPORAL
                },
                component_path.syntax_highlighted(ui.style()),
            ));
        }
        Item::DataResult(view_id, instance_path) => {
            // Break up in two:
            item_heading_no_breadcrumbs(ctx, viewport, ui, &Item::SpaceView(*view_id));
            separator_icon_ui(ui, icons::BREADCRUMBS_SEPARATOR);
            item_heading_no_breadcrumbs(
                ctx,
                viewport,
                ui,
                &Item::InstancePath(instance_path.clone()),
            );
        }
    }
}
