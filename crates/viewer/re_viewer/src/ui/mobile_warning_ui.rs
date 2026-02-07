pub fn mobile_warning_ui(ui: &mut egui::Ui) {
    // When running natively on Android, show the gRPC connection banner instead
    // of the mobile warning -- Android is a supported platform.
    #[cfg(target_os = "android")]
    {
        crate::ui::android_ui::grpc_connection_banner(ui);
        return;
    }

    #[cfg(not(target_os = "android"))]
    {
        use re_ui::{ContextExt as _, UiExt as _};

        let is_mobile_web = ui.ctx().os() == egui::os::OperatingSystem::IOS
            || ui.ctx().os() == egui::os::OperatingSystem::Android;

        if is_mobile_web {
            let frame = egui::Frame {
                fill: ui.visuals().panel_fill,
                ..ui.tokens().bottom_panel_frame()
            };

            egui::TopBottomPanel::bottom("warning_panel")
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
}
