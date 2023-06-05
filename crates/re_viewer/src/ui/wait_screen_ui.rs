use re_log_types::LogMsg;
use re_smart_channel::Receiver;

use itertools::Itertools as _;

pub fn wait_screen_ui(ui: &mut egui::Ui, rx: &Receiver<LogMsg>) {
    ui.centered_and_justified(|ui| {
        fn ready_and_waiting(ui: &mut egui::Ui, txt: &str) {
            let style = ui.style();
            let mut layout_job = egui::text::LayoutJob::default();
            layout_job.append(
                "Ready",
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Heading.resolve(style),
                    style.visuals.strong_text_color(),
                ),
            );
            layout_job.append(
                &format!("\n\n{txt}"),
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Body.resolve(style),
                    style.visuals.text_color(),
                ),
            );
            layout_job.halign = egui::Align::Center;
            ui.label(layout_job);
        }

        match rx.source() {
            re_smart_channel::SmartChannelSource::Files { paths } => {
                ui.strong(format!(
                    "Loading {}…",
                    paths
                        .iter()
                        .format_with(", ", |path, f| f(&format_args!("{}", path.display())))
                ));
            }
            re_smart_channel::SmartChannelSource::RrdHttpStream { url } => {
                ui.strong(format!("Loading {url}…"));
            }
            re_smart_channel::SmartChannelSource::RrdWebEventListener => {
                ready_and_waiting(ui, "Waiting for logging data…");
            }
            re_smart_channel::SmartChannelSource::Sdk => {
                ready_and_waiting(ui, "Waiting for logging data from SDK");
            }
            re_smart_channel::SmartChannelSource::WsClient { ws_server_url } => {
                // TODO(emilk): it would be even better to know whether or not we are connected, or are attempting to connect
                ready_and_waiting(ui, &format!("Waiting for data from {ws_server_url}"));
            }
            re_smart_channel::SmartChannelSource::TcpServer { port } => {
                ready_and_waiting(ui, &format!("Listening on port {port}"));
            }
        };
    });
}
