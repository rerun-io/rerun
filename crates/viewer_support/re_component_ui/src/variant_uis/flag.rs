use arrow::array::{Array as _, BooleanArray};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_types_core::{ComponentIdentifier, RowId};
use re_viewer_context::{AppContext, MaybeMutRef};

/// Render a scalar boolean as a flag and return its toggled value.
pub fn table_flag(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    _component: ComponentIdentifier,
    _row_id: Option<RowId>,
    value: &mut MaybeMutRef<'_, arrow::array::ArrayRef>,
) -> Result<egui::Response, Box<dyn std::error::Error>> {
    let bools = value
        .as_ref()
        .downcast_array_ref::<BooleanArray>()
        .ok_or("The table flag variant requires boolean data")?;
    if bools.len() != 1 {
        return Err("The table flag variant requires one scalar value".into());
    }

    let is_flagged = !bools.is_null(0) && bools.value(0);
    let enabled = value.as_mut().is_some();
    let mut response = ui
        .add_enabled_ui(enabled, |ui| flag_button(ui, is_flagged))
        .inner;

    if response.clicked()
        && let Some(value) = value.as_mut()
    {
        *value = std::sync::Arc::new(BooleanArray::from(vec![Some(!is_flagged)]));
        response.mark_changed();
    }
    Ok(response)
}

fn flag_button(ui: &mut egui::Ui, is_flagged: bool) -> egui::Response {
    use egui::NumExt as _;
    use re_ui::UiExt as _;

    /// Largest the flag icon gets.
    const ICON_SIZE: f32 = 16.0;

    /// Room between the icon and the edge of the button.
    const ICON_MARGIN: f32 = 3.0;

    let tokens = ui.tokens();

    // The button is as tall as `interact_size.y`, so it and its icon shrink to fit a table row.
    let height = ui
        .spacing()
        .interact_size
        .y
        .at_most(re_ui::FLAG_BUTTON_SIZE);
    let size = egui::vec2(re_ui::FLAG_BUTTON_SIZE, height);
    let icon_size = egui::Vec2::splat((height - 2.0 * ICON_MARGIN).clamp(0.0, ICON_SIZE));
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), is_flagged, "Flag")
    });

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() && ui.is_enabled();
        let (background, tint) = if is_flagged {
            // A set flag reads as a blue button, so it takes the blue button's colors.
            let blue = &tokens.button_blue;
            (
                if hovered {
                    blue.fill_hovered
                } else {
                    blue.fill
                },
                blue.text,
            )
        } else {
            (
                if hovered {
                    tokens.flag_untoggled_bg_hover
                } else {
                    tokens.flag_untoggled_bg
                },
                if hovered {
                    tokens.flag_untoggled_icon_hover
                } else {
                    tokens.flag_untoggled_icon
                },
            )
        };

        if background.a() > 0 {
            ui.painter().rect(
                rect,
                8.0,
                background,
                tokens.card_stroke,
                egui::StrokeKind::Inside,
            );
        }
        let icon = &re_ui::icons::FLAG_UNTOGGLED;
        icon.as_image()
            .tint(tint)
            .paint_at(ui, egui::Rect::from_center_size(rect.center(), icon_size));
    }

    response
}
