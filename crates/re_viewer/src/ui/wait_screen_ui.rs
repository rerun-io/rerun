use egui::Ui;
use itertools::Itertools;
use re_log_types::LogMsg;
use re_smart_channel::Receiver;

use re_ui::ReUi;

const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 400.0;

const PYTHON_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/python";
const CPP_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/cpp";
const RUST_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/rust";

/// Welcome screen shown in place of the viewport when no data is loaded.
pub fn welcome_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    rx: &Receiver<LogMsg>,
    command_sender: &re_viewer_context::CommandSender,
) {
    let wait_screen = WaitScreen::new();

    egui::ScrollArea::horizontal()
        .id_source("welcome screen")
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            wait_screen.show(re_ui, ui, rx, command_sender);
        });
}

/// Full-screen UI shown while in loading state.
pub fn loading_ui(ui: &mut egui::Ui, rx: &Receiver<LogMsg>) {
    ui.centered_and_justified(|ui| {
        let (status, source) = status_strings(rx);

        let style = ui.style();
        let mut layout_job = egui::text::LayoutJob::default();
        layout_job.append(
            status,
            0.0,
            egui::TextFormat::simple(
                egui::TextStyle::Heading.resolve(style),
                style.visuals.strong_text_color(),
            ),
        );
        layout_job.append(
            &format!("\n\n{source}"),
            0.0,
            egui::TextFormat::simple(
                egui::TextStyle::Body.resolve(style),
                style.visuals.text_color(),
            ),
        );
        layout_job.halign = egui::Align::Center;
        ui.label(layout_job);
    });
}

//TODO(ab): get rid of that unless we really need state here
pub struct WaitScreen {}

impl WaitScreen {
    pub fn new() -> Self {
        Self {}
    }

    #[allow(clippy::unused_self)]
    pub fn show(
        &self,
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        rx: &Receiver<LogMsg>,
        command_sender: &re_viewer_context::CommandSender,
    ) {
        let mut margin = egui::Margin::same(40.0);
        margin.bottom = 0.0;
        egui::Frame {
            inner_margin: margin,
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Welcome")
                            .strong()
                            .text_style(re_ui::ReUi::onboarding_h1()),
                    )
                    .wrap(false),
                );

                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Visualize multimodal data")
                            .text_style(re_ui::ReUi::onboarding_h2()),
                    )
                    .wrap(false),
                );

                ui.add_space(20.0);

                Self::onboarding_content_ui(re_ui, ui, command_sender);

                ui.add_space(55.0);

                let (status, source) = status_strings(rx);
                ui.vertical_centered(|ui| {
                    ui.label(status);
                    ui.label(egui::RichText::new(source).color(ui.visuals().weak_text_color()));
                });
            });
        });
    }

    fn onboarding_content_ui(
        re_ui: &ReUi,
        ui: &mut Ui,
        _command_sender: &re_viewer_context::CommandSender,
    ) {
        let column_spacing = 15.0;
        let column_width = ((ui.available_width() - 2. * column_spacing) / 3.0 - 1.0)
            .clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);

        let grid = egui::Grid::new("onboarding_grid")
            .spacing(egui::Vec2::splat(column_spacing))
            .min_col_width(column_width)
            .max_col_width(column_width);

        grid.show(ui, |ui| {
            image_banner(re_ui, ui, &re_ui::icons::ONBOARDING_LIVE_DATA, column_width);
            image_banner(
                re_ui,
                ui,
                &re_ui::icons::ONBOARDING_RECORDED_DATA,
                column_width,
            );
            image_banner(re_ui, ui, &re_ui::icons::ONBOARDING_CONFIGURE, column_width);

            ui.end_row();

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Connect to live data")
                        .strong()
                        .text_style(re_ui::ReUi::onboarding_h3()),
                );
                ui.label(
                    egui::RichText::new(
                        "Use the Rerun SDK to stream data from your code to the Rerun Viewer. \
                        Visualize synchronized data from multiple processes, locally or over a \
                        network.",
                    )
                    .text_style(re_ui::ReUi::onboarding_body()),
                );
            });

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Load recorded data")
                        .strong()
                        .text_style(re_ui::ReUi::onboarding_h3()),
                );
                ui.label(
                    egui::RichText::new(
                        "Open and visualize recorded data from previous Rerun sessions (.rrd) as \
                        well as data in formats like .gltf and .jpg.",
                    )
                    .text_style(re_ui::ReUi::onboarding_body()),
                );
            });

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Configure your views")
                        .strong()
                        .text_style(re_ui::ReUi::onboarding_h3()),
                );
                ui.label(
                    egui::RichText::new(
                        "Add and rearrange views, and configure what data is shown and how. \
                        Configure interactively in the viewer or (coming soon) directly from code \
                        in the SDK.",
                    )
                    .text_style(re_ui::ReUi::onboarding_body()),
                );
            });

            ui.end_row();

            ui.horizontal(|ui| {
                button_centered_label(ui, "Quick start...");
                url_large_text_buttons(ui, "Python", PYTHON_QUICKSTART);
                url_large_text_buttons(ui, "C++", CPP_QUICKSTART);
                url_large_text_buttons(ui, "Rust", RUST_QUICKSTART);
            });

            #[cfg(not(target_arch = "wasm32"))]
            {
                use re_ui::UICommandSender;
                ui.horizontal(|ui| {
                    if large_text_buttons(ui, "Open file...").clicked() {
                        _command_sender.send_ui(re_ui::UICommand::Open);
                    }
                    button_centered_label(ui, "Or drop a file anywhere!");
                });
            }

            #[cfg(target_arch = "wasm32")]
            ui.horizontal(|ui| {
                button_centered_label(ui, "Drop a file anywhere!");
            });

            ui.horizontal(|ui| {
                large_text_buttons(ui, "Add View");
            });

            ui.end_row();
        });
    }
}

fn button_centered_label(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) {
    ui.vertical(|ui| {
        ui.add_space(9.0);
        ui.label(label);
    });
}

fn url_large_text_buttons(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, url: &str) {
    if large_text_buttons(ui, text).clicked() {
        ui.ctx().output_mut(|o| {
            o.open_url = Some(egui::output::OpenUrl {
                url: url.to_owned(),
                new_tab: true,
            });
        });
    }
}

fn large_text_buttons(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = egui::vec2(12.0, 9.0);
        let visuals = ui.visuals_mut();
        visuals.widgets.hovered.expansion = 0.0;
        visuals.widgets.active.expansion = 0.0;
        visuals.widgets.open.expansion = 0.0;

        visuals.widgets.inactive.rounding = egui::Rounding::same(8.);
        visuals.widgets.hovered.rounding = egui::Rounding::same(8.);
        visuals.widgets.active.rounding = egui::Rounding::same(8.);

        visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;

        ui.button(text)
    })
    .inner
}

fn image_banner(re_ui: &re_ui::ReUi, ui: &mut egui::Ui, image: &re_ui::Icon, column_width: f32) {
    let image = re_ui.icon_image(image);
    let texture_id = image.texture_id(ui.ctx());
    let height = column_width * image.size()[1] as f32 / image.size()[0] as f32;
    ui.add(
        egui::Image::new(texture_id, egui::vec2(column_width, height))
            .rounding(egui::Rounding::same(8.)),
    );
}

pub fn status_strings(rx: &Receiver<LogMsg>) -> (&'static str, String) {
    match rx.source() {
        re_smart_channel::SmartChannelSource::Files { paths } => (
            "Loading…",
            format!(
                "{}",
                paths
                    .iter()
                    .format_with(", ", |path, f| f(&format_args!("{}", path.display())))
            ),
        ),
        re_smart_channel::SmartChannelSource::RrdHttpStream { url } => ("Loading…", url.clone()),
        re_smart_channel::SmartChannelSource::RrdWebEventListener => {
            ("Ready", "Waiting for logging data…".to_owned())
        }
        re_smart_channel::SmartChannelSource::Sdk => {
            ("Ready", "Waiting for logging data from SDK".to_owned())
        }
        re_smart_channel::SmartChannelSource::WsClient { ws_server_url } => {
            // TODO(emilk): it would be even better to know whether or not we are connected, or are attempting to connect
            ("Ready", format!("Waiting for data from {ws_server_url}"))
        }
        re_smart_channel::SmartChannelSource::TcpServer { port } => {
            ("Ready", format!("Listening on port {port}"))
        }
    }
}
