use re_sdk_types::datatypes::Rgba32;
use re_viewer_context::MaybeMutRef;

pub fn edit_rgba32(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Rgba32>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, Rgba32> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_rgba32_impl(ui, &mut value)
}

fn edit_rgba32_impl(ui: &mut egui::Ui, color: &mut MaybeMutRef<'_, Rgba32>) -> egui::Response {
    let response = if let Some(color) = color.as_mut() {
        let mut edit_color = egui::Color32::from(*color);
        let response = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut edit_color,
            // TODO(andreas): It would be nice to be explicit about the semantics here and enable alpha only when it has an effect.
            egui::color_picker::Alpha::OnlyBlend,
        );
        *color = edit_color.into();
        response
    } else {
        let [r, g, b, a] = color.to_array();
        #[expect(clippy::disallowed_methods)] // This is not a hard-coded color.
        let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        egui::color_picker::show_color(ui, color, egui::Vec2::new(32.0, 16.0))
    };

    ui.painter().rect_stroke(
        response.rect,
        1.0,
        ui.visuals().widgets.noninteractive.fg_stroke,
        egui::StrokeKind::Inside,
    );

    let [r, g, b, a] = color.to_array();
    response.on_hover_ui(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.monospace(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"));
    })
}
