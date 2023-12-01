mod example_page;
mod welcome_page;

use egui::Widget;
use re_log_types::LogMsg;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_ui::ReUi;
use std::hash::Hash;
use welcome_page::welcome_page_ui;

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

#[derive(Default)]
#[must_use]
pub(super) struct WelcomeScreenResponse {
    pub go_to_example_page: bool,
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
    pub fn set_examples_manifest_url(&mut self, url: String) {
        self.example_page.set_manifest_url(url);
    }

    /// Welcome screen shown in place of the viewport when no data is loaded.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        re_ui: &re_ui::ReUi,
        rx: &ReceiveSet<LogMsg>,
        command_sender: &re_viewer_context::CommandSender,
    ) {
        // tab bar
        egui::Frame {
            inner_margin: egui::Margin::symmetric(12.0, 8.0),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ReUi::welcome_screen_tab_bar_style(ui);

                ui.selectable_value(
                    &mut self.current_page,
                    WelcomeScreenPage::Welcome,
                    "Welcome",
                );
                ui.selectable_value(
                    &mut self.current_page,
                    WelcomeScreenPage::Examples,
                    "Examples",
                );
            });
        });

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
                    top: 16.0,
                    bottom: 8.0,
                };
                egui::Frame {
                    inner_margin: margin,
                    ..Default::default()
                }
                .show(ui, |ui| match self.current_page {
                    WelcomeScreenPage::Welcome => welcome_page_ui(ui, rx, command_sender),
                    WelcomeScreenPage::Examples => {
                        self.example_page.ui(ui, re_ui, rx, command_sender)
                    }
                })
                .inner
            })
            .inner;

        if response.go_to_example_page {
            self.current_page = WelcomeScreenPage::Examples;
        }
    }
}

/// Full-screen UI shown while in loading state.
pub fn loading_ui(ui: &mut egui::Ui, rx: &ReceiveSet<LogMsg>) {
    let status_strings = status_strings(rx);
    if status_strings.is_empty() {
        return;
    }

    ui.centered_and_justified(|ui| {
        for status_string in status_strings {
            let style = ui.style();
            let mut layout_job = egui::text::LayoutJob::default();
            layout_job.append(
                status_string.status,
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Heading.resolve(style),
                    style.visuals.strong_text_color(),
                ),
            );
            layout_job.append(
                &format!("\n\n{}", status_string.source),
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Body.resolve(style),
                    style.visuals.text_color(),
                ),
            );
            layout_job.halign = egui::Align::Center;
            ui.label(layout_job);
        }
    });
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

/// Describes the current state of the Rerun viewer.
struct StatusString {
    /// General status string (e.g. "Ready", "Loading…", etc.).
    status: &'static str,

    /// Source string (e.g. listening IP, file path, etc.).
    source: String,

    /// Whether or not the status is valid once data loading is completed, i.e. if data may still
    /// be received later.
    long_term: bool,
}

impl StatusString {
    fn new(status: &'static str, source: String, long_term: bool) -> Self {
        Self {
            status,
            source,
            long_term,
        }
    }
}

/// Returns the status strings to be displayed by the loading and welcome screen.
fn status_strings(rx: &ReceiveSet<LogMsg>) -> Vec<StatusString> {
    rx.sources()
        .into_iter()
        .map(|s| status_string(&s))
        .collect()
}

fn status_string(source: &SmartChannelSource) -> StatusString {
    match source {
        re_smart_channel::SmartChannelSource::File(path) => {
            StatusString::new("Loading…", path.display().to_string(), false)
        }
        re_smart_channel::SmartChannelSource::RrdHttpStream { url } => {
            StatusString::new("Loading…", url.clone(), false)
        }
        re_smart_channel::SmartChannelSource::RrdWebEventListener => {
            StatusString::new("Ready", "Waiting for logging data…".to_owned(), true)
        }
        re_smart_channel::SmartChannelSource::Sdk => StatusString::new(
            "Ready",
            "Waiting for logging data from SDK".to_owned(),
            true,
        ),
        re_smart_channel::SmartChannelSource::WsClient { ws_server_url } => {
            // TODO(emilk): it would be even better to know whether or not we are connected, or are attempting to connect
            StatusString::new(
                "Ready",
                format!("Waiting for data from {ws_server_url}"),
                true,
            )
        }
        re_smart_channel::SmartChannelSource::TcpServer { port } => {
            StatusString::new("Ready", format!("Listening on port {port}"), true)
        }
    }
}
