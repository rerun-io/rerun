use re_types::components::RecordingUri;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn singleline_view_recording_uri(
    ctx: &ViewerContext<'_>,
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

        if response.clicked() {
            let data_source = re_data_source::DataSource::from_uri(
                re_log_types::FileSource::Uri,
                value.uri().to_owned(),
            );

            let cmd_sender = ctx.command_sender.clone();
            let on_cmd = Box::new(move |cmd| match cmd {
                re_grpc_client::redap::Command::SetLoopSelection {
                    recording_id,
                    time_range,
                } => cmd_sender.send_system(SystemCommand::SetLoopSelection {
                    rec_id: re_log_types::StoreId::from_string(
                        re_log_types::StoreKind::Recording,
                        recording_id,
                    ),
                    timeline: time_range.timeline,
                    time_range: time_range.time_range,
                }),
            });

            match data_source.stream(on_cmd, None) {
                Ok(rx) => ctx
                    .command_sender
                    .send_system(SystemCommand::AddReceiver(rx)),
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
