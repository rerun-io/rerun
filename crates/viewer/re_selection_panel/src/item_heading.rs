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

use re_ui::{list_item, DesignTokens, UiExt as _};
use re_viewer_context::{Item, SystemCommandSender as _, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

use crate::ItemTitle;

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
    let item_title = ItemTitle::from_item(ctx, viewport, ui.style(), item);

    let ItemTitle {
        name,
        tooltip,
        icon,
        label_style,
    } = item_title;

    let response = ui.add(egui::Button::image_and_text(icon.as_image(), name));

    if response.clicked() {
        // If the user has multiple things selected but only wants to have one thing selected,
        // this is how they can do it.
        ctx.command_sender
            .send_system(re_viewer_context::SystemCommand::SetSelection(item.clone()));
    }

    if let Some(tooltip) = tooltip {
        response.on_hover_text(tooltip);
    }
}
