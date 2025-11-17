use re_data_ui::item_ui::timeline_button;
use re_types::{blueprint::components::TimelineName, datatypes};
use re_viewer_context::MaybeMutRef;

pub fn timeline_name_singleline_edit_or_view_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, TimelineName>,
) -> egui::Response {
    match value {
        MaybeMutRef::Ref(value) => timeline_button(ctx, ui, &(*value).into()),
        MaybeMutRef::MutRef(value) => {
            let mut any_edit = false;
            let egui::InnerResponse { mut response, .. } =
                egui::ComboBox::from_id_salt("timeline_name")
                    .selected_text(value.as_str())
                    .show_ui(ui, |ui| {
                        for timeline in ctx.recording().times_per_timeline().timelines() {
                            any_edit |= ui
                                .selectable_value(
                                    &mut value.0,
                                    datatypes::Utf8::from(timeline.name().as_str()),
                                    timeline.name().as_str(),
                                )
                                .changed();
                        }
                    });

            if any_edit {
                response.mark_changed();
            }

            response
        }
    }
}
