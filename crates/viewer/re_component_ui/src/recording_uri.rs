use re_types::components::RecordingUri;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn singleline_view_recording_uri(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, RecordingUri>,
) -> egui::Response {
    let value = value.as_ref();

    #[cfg(not(target_arch = "wasm32"))]
    {
        use re_viewer_context::{SystemCommand, SystemCommandSender};

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

        if response.clicked() || response.middle_clicked() {
            let do_no_switch_to_viewer =
                response.middle_clicked() || ui.input(|i| i.modifiers.command);

            let data_source = re_data_source::DataSource::from_uri(
                re_log_types::FileSource::Uri,
                value.uri().to_owned(),
            );

            match data_source.stream(None) {
                Ok(re_data_source::StreamSource::LogMessages(rx)) => _ctx
                    .command_sender()
                    .send_system(SystemCommand::AddReceiver {
                        rx,
                        switch_to_viewer: !do_no_switch_to_viewer,
                    }),
                Ok(re_data_source::StreamSource::CatalogData { origin: url }) => {
                    // TODO(antoine, andreas): This branch might become relevant in the future.
                    re_log::warn!("Recording URI was formatted like a catalog URI: {url}");
                }
                Err(err) => re_log::warn!("Could not open recording URI: {err}"),
            }
        }

        response
    }

    #[cfg(target_arch = "wasm32")]
    {
        re_viewer_context::UiLayout::List.label(ui, value.uri())
    }
}
