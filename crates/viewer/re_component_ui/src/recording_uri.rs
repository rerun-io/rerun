use re_types::components::RecordingUri;
use re_viewer_context::{MaybeMutRef, SystemCommand, SystemCommandSender, ViewerContext};

pub fn singleline_view_recording_uri(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, RecordingUri>,
) -> egui::Response {
    let value = value.as_ref();

    let response = ui
        .scope(|ui| {
            if ui.style().wrap_mode.is_none() {
                ui.style_mut().wrap_mode = Some(if ui.is_sizing_pass() {
                    egui::TextWrapMode::Extend
                } else {
                    egui::TextWrapMode::Truncate
                });
            }

            ui.link(value.uri())
        })
        .inner;

    if response.clicked() {
        let data_source = re_data_source::DataSource::from_uri(
            re_log_types::FileSource::Uri,
            value.uri().to_owned(),
        );

        match data_source.stream(None) {
            Ok(rx) => ctx
                .command_sender
                .send_system(SystemCommand::AddReceiver(rx)),
            Err(err) => re_log::warn!("Could not open recording URI: {err}"),
        }
    }

    response
}
