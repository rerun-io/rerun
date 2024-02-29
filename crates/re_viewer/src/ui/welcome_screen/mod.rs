mod example_section;
mod welcome_section;

use egui::Widget;
use example_section::ExampleSection;
use welcome_section::welcome_section_ui;

use re_log_types::LogMsg;
use re_smart_channel::ReceiveSet;
use re_ui::ReUi;

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
        rx: &ReceiveSet<LogMsg>,
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
                    welcome_section_ui(ui, rx, command_sender);
                    ui.add_space(80.0);
                    self.example_page.ui(ui, re_ui, command_sender);
                });
            });
    }
}

fn set_large_button_style(ui: &mut egui::Ui) {
    ui.style_mut().spacing.button_padding = egui::vec2(10.0, 7.0);
    let visuals = ui.visuals_mut();
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.expansion = 0.0;
    visuals.widgets.open.expansion = 0.0;

    visuals.widgets.inactive.rounding = egui::Rounding::same(8.);
    visuals.widgets.hovered.rounding = egui::Rounding::same(8.);
    visuals.widgets.active.rounding = egui::Rounding::same(8.);

    visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;
}

fn url_large_text_button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, url: &str) {
    ui.scope(|ui| {
        set_large_button_style(ui);

        if egui::Button::image_and_text(
            re_ui::icons::EXTERNAL_LINK
                .as_image()
                .fit_to_exact_size(ReUi::small_icon_size()),
            text,
        )
        .ui(ui)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
        {
            ui.ctx().output_mut(|o| {
                o.open_url = Some(egui::output::OpenUrl {
                    url: url.to_owned(),
                    new_tab: true,
                });
            });
        }
    });
}

fn large_text_button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.scope(|ui| {
        set_large_button_style(ui);
        ui.button(text)
    })
    .inner
}
