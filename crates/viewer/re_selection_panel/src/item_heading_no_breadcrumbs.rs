use egui::WidgetText;

use re_data_ui::item_ui::guess_instance_path_icon;
use re_log_types::ComponentPath;
use re_ui::{icons, Icon, SyntaxHighlighting};
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
                tooltip: _,
            } = ItemTitle::from_item(ctx, viewport, ui.style(), item);

            icon_and_title(ui, icon, label);
        }
        Item::InstancePath(instance_path) => {
            icon_and_title(
                ui,
                guess_instance_path_icon(ctx, instance_path),
                instance_path.syntax_highlighted(ui.style()),
            );
        }
        Item::ComponentPath(component_path) => {
            let is_component_static = is_component_static(ctx, component_path);

            // Break up into entity path and component name:
            let ComponentPath {
                entity_path,
                component_name,
            } = component_path;

            item_heading_no_breadcrumbs(ctx, viewport, ui, &Item::from(entity_path.clone()));

            separator_icon_ui(ui);

            let component_icon = if is_component_static {
                &icons::COMPONENT_STATIC
            } else {
                &icons::COMPONENT_TEMPORAL
            };
            icon_and_title(
                ui,
                component_icon,
                component_name.syntax_highlighted(ui.style()),
            );
        }
        Item::DataResult(view_id, instance_path) => {
            // Break up into view and instance path:
            item_heading_no_breadcrumbs(ctx, viewport, ui, &Item::SpaceView(*view_id));
            separator_icon_ui(ui);
            item_heading_no_breadcrumbs(
                ctx,
                viewport,
                ui,
                &Item::InstancePath(instance_path.clone()),
            );
        }
    }
}

fn icon_and_title(ui: &mut egui::Ui, icon: &Icon, title: impl Into<WidgetText>) {
    ui.add(icon.as_image());
    ui.label(title);
}
