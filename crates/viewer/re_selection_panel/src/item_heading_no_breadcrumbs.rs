use egui::WidgetText;
use re_data_ui::item_ui::cursor_interact_with_selectable;
use re_ui::{Icon, UiExt as _, list_item};
use re_viewer_context::{Item, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use re_data_ui::item_title::{QualifiedItemTitle, part_separator_image};

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
    let title = QualifiedItemTitle::from_item(ctx, viewport, ui.style(), item);

    for (i, part) in title.parts.into_iter().enumerate() {
        if i != 0 {
            ui.add(part_separator_image(icon_tint));
        }
        icon_and_title(ui, part.icon, part.label, icon_tint);
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
