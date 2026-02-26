use re_sdk_types::datatypes::Rgba32;
use re_ui::UiExt as _;
use re_viewer_context::MaybeMutRef;

use crate::color_swatch::ColorSwatch;

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

fn edit_rgba32_impl<'a>(
    ui: &mut egui::Ui,
    color: &'a mut MaybeMutRef<'a, Rgba32>,
) -> egui::Response {
    ui.add(ColorSwatch::new(color))
}

pub fn edit_rgba32_array(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    colors: &mut MaybeMutRef<'_, Vec<re_sdk_types::components::Color>>,
) -> egui::Response {
    const MAX_COLORS_TO_SHOW: usize = 16;

    // TODO(andreas): we have the technical limitation right now that we always write out the entire array for edits.
    // This means that if you e.g. have a point cloud of a million points with individual colors, every single edit will write out all million colors again.
    const MAX_NUM_COLORS_FOR_EDITING: usize = 64;

    let response = ui
        .horizontal(|ui| {
            let num_colors = colors.len();
            ui.spacing_mut().item_spacing.x = 4.0;

            // Calculate how many colors we can fit based on available width. Subtract 25
            // to leave space for the …(8) text
            let available_width = ui.available_width() - 25.0;
            let gap = ui.spacing().item_spacing.x;

            let color_swatch_width: f32 = ui.tokens().color_swatch_size;
            let max_colors_that_fit =
                (available_width / (color_swatch_width + gap)).floor() as usize;
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
