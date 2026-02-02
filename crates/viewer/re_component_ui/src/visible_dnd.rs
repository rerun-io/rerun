use std::hash::Hash;

use re_ui::{HasDesignTokens as _, UiExt as _};

/// A shared utility for a drag and drop ui with a visibility button for each item.
pub fn visible_dnd<T: Hash>(
    ui: &mut egui::Ui,
    id_source: impl Hash,
    items: &mut [T],
    mut item_ui: impl FnMut(&mut egui::Ui, &mut T),
    mut get_item_visibility: impl FnMut(&T) -> bool,
    mut set_item_visibility: impl FnMut(&mut T, bool),
) -> egui::Response {
    let mut any_edit = false;

    const ITEM_SPACING: f32 = 8.0;
    let egui::InnerResponse { mut response, .. } = egui::Frame::new()
        .corner_radius(ui.visuals().menu_corner_radius)
        .fill(ui.visuals().tokens().text_edit_bg_color)
        .inner_margin(egui::Margin {
            left: ITEM_SPACING as i8,
            right: ITEM_SPACING as i8,
            top: ITEM_SPACING as i8,
            bottom: (ITEM_SPACING * 0.5) as i8,
        })
        .show(ui, |ui| {
            let text_height = ui
                .style()
                .text_styles
                .get(&egui::TextStyle::Body)
                .map(|s| s.size)
                .unwrap_or(0.0);
            let sz = egui::vec2(ui.max_rect().size().x, ITEM_SPACING + text_height);
            let dnd_res = egui_dnd::dnd(ui, id_source).show_sized(
                // We include the index in the item here because the item
                // so doing this will make columns with the
                // same name not collide.
                items.iter_mut().enumerate(),
                sz,
                |ui, (_idx, item), handle, _state| {
                    ui.horizontal(|ui| {
                        handle.ui(ui, |ui| {
                            ui.small_icon(
                                &re_ui::icons::DND_HANDLE,
                                Some(ui.visuals().text_color()),
                            );
                        });

                        let mut visible = get_item_visibility(item);

                        egui::containers::Sides::new().shrink_left().show(
                            ui,
                            |ui| item_ui(ui, item),
                            |ui| {
                                any_edit |= ui.visibility_toggle_button(&mut visible).changed();
                            },
                        );

                        set_item_visibility(item, visible);
                    });
                },
            );

            if dnd_res.is_drag_finished() {
                any_edit = true;
                dnd_res.update_vec(items);
            }
        });

    if any_edit {
        response.mark_changed();
    }

    response
}
