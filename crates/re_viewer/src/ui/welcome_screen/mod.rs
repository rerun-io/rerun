mod example_section;
mod welcome_section;

use example_section::ExampleSection;
use welcome_section::welcome_section_ui;

#[derive(Default)]
pub struct WelcomeScreen {
    example_page: ExampleSection,
}

impl WelcomeScreen {
    pub fn set_examples_manifest_url(&mut self, egui_ctx: &egui::Context, url: String) {
        self.example_page.set_manifest_url(egui_ctx, url);
    }

    /// Welcome screen shown in place of the viewport when no data is loaded.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        re_ui: &re_ui::ReUi,
        command_sender: &re_viewer_context::CommandSender,
    ) {
        // This is needed otherwise `example_page_ui` bleeds by a few pixels over the timeline panel
        // TODO(ab): figure out why that happens
        ui.set_clip_rect(ui.available_rect_before_wrap());

        egui::ScrollArea::vertical()
            .id_source("welcome_screen_page")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame {
                    inner_margin: egui::Margin {
                        left: 40.0,
                        right: 40.0,
                        top: 32.0,
                        bottom: 8.0,
                    },
                    ..Default::default()
                }
                .show(ui, |ui| {
                    welcome_section_ui(ui);
                    ui.add_space(83.0);
                    self.example_page.ui(ui, re_ui, command_sender);
                });
            });
    }
}
