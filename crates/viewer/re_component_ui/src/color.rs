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

pub fn edit_rgba32_array(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    colors: &mut MaybeMutRef<'_, Vec<re_sdk_types::components::Color>>,
) -> egui::Response {
    const COLOR_SWATCH_WIDTH: f32 = 40.0;
    const MAX_COLORS_TO_SHOW: usize = 16;

    // TODO(andreas): we have the technical limitation right now that we always write out the entire array for edits.
    // This means that if you e.g. have a point cloud of a million points with individual colors, every single edit will write out all million colors again.
    const MAX_NUM_COLORS_FOR_EDITING: usize = 64;

    let response = ui
        .horizontal(|ui| {
            let num_colors = colors.len();

            // Calculate how many colors we can fit based on available width. Subtract 25
            // to leave space for the …(8) text
            let available_width = ui.available_width() - 25.0;
            let gap = ui.spacing().item_spacing.x;

            let max_colors_that_fit =
                (available_width / (COLOR_SWATCH_WIDTH + gap)).floor() as usize;
            let colors_to_show = num_colors.min(max_colors_that_fit.clamp(1, MAX_COLORS_TO_SHOW));

            let mut accumulated_response: Option<egui::Response> = None;

            for i in 0..colors_to_show {
                let mut color_rgba32 = match colors {
                    MaybeMutRef::Ref(colors) => MaybeMutRef::Ref(&colors[i].0),
                    MaybeMutRef::MutRef(colors) => {
                        if colors.len() > MAX_NUM_COLORS_FOR_EDITING {
                            // Too many colors to edit, show as read-only:
                            MaybeMutRef::Ref(&colors[i].0)
                        } else {
                            MaybeMutRef::MutRef(&mut colors[i].0)
                        }
                    }
                };
                let color_response = edit_rgba32_impl(ui, &mut color_rgba32);
                accumulated_response = Some(match accumulated_response {
                    Some(prev) => prev.union(color_response),
                    None => color_response,
                });
            }

            if num_colors > colors_to_show {
                ui.weak(format!("…({num_colors})"));
            }

            accumulated_response
        })
        .inner;

    response.unwrap_or_else(|| ui.label(""))
}
