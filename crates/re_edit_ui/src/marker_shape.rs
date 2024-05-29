use egui::NumExt as _;

use re_types::components::MarkerShape;
use re_viewer_context::ViewerContext;

pub(crate) fn edit_marker_shape_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    edit_marker: &mut MarkerShape,
) -> egui::Response {
    let marker_text = edit_marker.to_string(); // TODO(emilk): Show marker shape in the selected text

    let item_width = 100.0;

    let outer_response = egui::ComboBox::from_id_source("marker_shape")
        .selected_text(marker_text)
        .width(
            ui.available_width()
                .at_most(item_width + ui.spacing().menu_margin.sum().x),
        )
        .height(320.0)
        .show_ui(ui, |ui| {
            // workaround to force `ui.max_rect()` to reflect the content size
            ui.set_width(item_width);

            let background_x_range = (ui.max_rect() + ui.spacing().menu_margin).x_range();

            let list_ui = |ui: &mut egui::Ui| {
                let mut combined_response: Option<egui::Response> = None;
                for marker in MarkerShape::ALL {
                    let mut response = ctx
                        .re_ui
                        .list_item()
                        .selected(*edit_marker == marker)
                        .show_flat(
                            ui,
                            re_ui::list_item::LabelContent::new(marker.to_string())
                                .min_desired_width(item_width)
                                .with_icon_fn(|_re_ui, ui, rect, visuals| {
                                    paint_marker(ui, marker.into(), rect, visuals.text_color());
                                }),
                        );

                    if response.clicked() {
                        *edit_marker = marker;
                        response.changed = true;
                    }

                    combined_response = Some(match combined_response {
                        Some(combined_response) => combined_response.union(response),
                        None => response,
                    });
                }
                combined_response.expect("At least one marker shape should be available")
            };

            re_ui::full_span::full_span_scope(ui, background_x_range, |ui| {
                re_ui::list_item::list_item_scope(ui, "marker_shape", list_ui)
            })
        });

    outer_response.inner.unwrap_or(outer_response.response)
}

pub(crate) fn paint_marker(
    ui: &egui::Ui,
    marker: egui_plot::MarkerShape,
    rect: egui::Rect,
    color: egui::Color32,
) {
    use egui_plot::PlotItem as _;

    let points = egui_plot::Points::new([0.0, 0.0])
        .shape(marker)
        .color(color)
        .radius(rect.size().min_elem() / 2.0)
        .filled(true);

    let bounds = egui_plot::PlotBounds::new_symmetrical(0.5);
    let transform = egui_plot::PlotTransform::new(rect, bounds, true, true);

    let mut shapes = vec![];
    points.shapes(ui, &transform, &mut shapes);
    ui.painter().extend(shapes);
}
