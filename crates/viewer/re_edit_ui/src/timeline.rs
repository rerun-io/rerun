use re_types::blueprint::components;
use re_types_core::LoggableBatch as _;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub(crate) fn edit_timeline(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, components::Timeline>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let mut current_value = value.timeline_name();
        let id_source = value.name();
        let mut changed = false;
        let mut combobox_response = egui::ComboBox::from_id_source(id_source)
            .selected_text(current_value.as_str())
            .show_ui(ui, |ui| {
                for timeline in ctx.recording().timelines() {
                    let response = ui.selectable_value(
                        &mut current_value,
                        *timeline.name(),
                        timeline.name().as_str(),
                    );

                    changed |= response.changed();
                }
            });

        if changed {
            value.set_timeline_name(current_value);
            combobox_response.response.mark_changed();
        }

        combobox_response.response
    } else {
        ui.label(value.timeline_name().as_str())
    }
}
