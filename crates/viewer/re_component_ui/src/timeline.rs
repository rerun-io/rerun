use re_types::blueprint::components;
use re_types_core::LoggableBatch as _;
use re_viewer_context::external::re_log_types::TimelineName;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub(crate) fn edit_timeline_name(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, components::TimelineName>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let mut current_value: TimelineName = (&*value).into();
        let id_salt = value.name();
        let mut changed = false;
        let mut combobox_response = egui::ComboBox::from_id_salt(id_salt)
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
            *value = current_value.as_str().into();
            combobox_response.response.mark_changed();
        }

        combobox_response.response
    } else {
        ui.label(value.as_str())
    }
}
