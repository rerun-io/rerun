use re_ui::{ContextExt as _, UiExt as _};

pub fn mobile_warning_ui(ui: &mut egui::Ui) {
    // We have not yet optimized the UI experience for mobile. Show a warning banner
    // with a link to the tracking issue.

    if ui.os() == egui::os::OperatingSystem::IOS || ui.os() == egui::os::OperatingSystem::Android {
        let frame = egui::Frame {
            fill: ui.visuals().panel_fill,
            ..ui.tokens().bottom_panel_frame()
        };

        egui::Panel::bottom("warning_panel")
            .resizable(false)
            .frame(frame)
            .show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    let text = ui
                        .ctx()
                        .warning_text("Mobile OSes are not yet supported. Click for details.");
                    ui.hyperlink_to(text, "https://github.com/rerun-io/rerun/issues/1672");
                });
            });
    }
}
