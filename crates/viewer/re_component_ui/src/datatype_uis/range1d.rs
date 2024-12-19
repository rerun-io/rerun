use egui::NumExt as _;

use re_types::datatypes::Range1D;
use re_viewer_context::{MaybeMutRef, UiLayout};

pub fn edit_view_range1d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Range1D>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, Range1D> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_view_range1d_impl(ui, &mut value)
}

fn edit_view_range1d_impl(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Range1D>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let [min, max] = &mut value.0;
        let range = (*max - *min).abs();
        let speed = (range * 0.01).at_least(0.001);

        let response_min = ui.add(
            egui::DragValue::new(min)
                .clamp_existing_to_range(false)
                .range(f64::NEG_INFINITY..=*max)
                .speed(speed),
        );
        ui.label("-");
        let response_max = ui.add(
            egui::DragValue::new(max)
                .clamp_existing_to_range(false)
                .range(*min..=f64::INFINITY)
                .speed(speed),
        );

        response_min | response_max
    } else {
        let [min, max] = value.0;
        UiLayout::List.data_label(
            ui,
            format!(
                "{} - {}",
                re_format::format_f64(min),
                re_format::format_f64(max)
            ),
        )
    }
}
