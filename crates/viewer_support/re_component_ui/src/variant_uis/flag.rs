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
    use re_ui::UiExt as _;

    let tokens = ui.tokens();
    let size = egui::vec2(30.0, 24.0);
    let icon_size = egui::vec2(14.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            is_flagged,
            "Flag",
        )
    });

    // Three visual tiers based on hover context:
    // - **Idle** (mouse away from card): transparent bg, muted icon — flag "melts" into the card.
    // - **Card hovered**: subtle bg appears, icon becomes legible — flag is *revealed*.
    // - **Flag hovered**: stronger bg, same icon — flag is clearly *actionable*.
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() && ui.is_enabled();
        let (background, tint) = if is_flagged {
            (
                if hovered {
                    tokens.flag_toggled_bg_hover
                } else {
                    tokens.flag_toggled_bg
                },
                tokens.flag_toggled_icon,
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
            ui.painter().rect_filled(rect, 4.0, background);
        }
        let icon = if is_flagged {
            &re_ui::icons::FLAG_TOGGLED
        } else {
            &re_ui::icons::FLAG_UNTOGGLED
        };
        icon.as_image()
            .tint(tint)
            .paint_at(ui, egui::Rect::from_center_size(rect.center(), icon_size));
    }

    response
}
