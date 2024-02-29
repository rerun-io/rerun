mod example_page;
mod welcome_page;

use std::hash::Hash;

use egui::Widget;
use welcome_page::welcome_page_ui;

use re_log_types::LogMsg;
use re_smart_channel::ReceiveSet;
use re_ui::ReUi;

#[derive(Debug, Default, PartialEq, Hash)]
enum WelcomeScreenPage {
    #[default]
    Welcome,
    Examples,
}

pub struct WelcomeScreen {
    current_page: WelcomeScreenPage,
    example_page: example_page::ExamplePage,
}

#[derive(Clone, Copy, Default)]
#[must_use]
pub(super) struct WelcomeScreenResponse {
    pub go_to_example_page: bool,
}

impl WelcomeScreenResponse {
    fn merge_with(self, other: WelcomeScreenResponse) -> Self {
        Self {
            go_to_example_page: self.go_to_example_page || other.go_to_example_page,
        }
    }
}

impl Default for WelcomeScreen {
    fn default() -> Self {
        Self {
            current_page: WelcomeScreenPage::Welcome,
            example_page: example_page::ExamplePage::default(),
        }
    }
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

        let response: WelcomeScreenResponse = egui::ScrollArea::vertical()
            .id_source(("welcome_screen_page", &self.current_page))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let margin = egui::Margin {
                    left: 40.0,
                    right: 40.0,
                    top: 24.0,
                    bottom: 8.0,
                };
                egui::Frame {
                    inner_margin: margin,
                    ..Default::default()
                }
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    let response = welcome_page_ui(ui, rx, command_sender);
                    ui.add_space(80.0);
                    self.example_page
                        .ui(ui, re_ui, command_sender)
                        .merge_with(response)
                })
                .inner
            })
            .inner;

        if response.go_to_example_page {
            self.current_page = WelcomeScreenPage::Examples;
        }
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
