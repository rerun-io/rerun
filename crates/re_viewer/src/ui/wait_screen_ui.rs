use egui::{Ui, Widget};

use re_log_types::LogMsg;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_ui::ReUi;

const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 400.0;

//const CPP_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/cpp";
const PYTHON_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/python";
const RUST_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/rust";
const SPACE_VIEWS_HELP: &str = "https://www.rerun.io/docs/getting-started/viewer-walkthrough";

/// Welcome screen shown in place of the viewport when no data is loaded.
pub fn welcome_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    rx: &ReceiveSet<LogMsg>,
    command_sender: &re_viewer_context::CommandSender,
) {
    egui::ScrollArea::both()
        .id_source("welcome screen")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            welcome_ui_impl(re_ui, ui, rx, command_sender);
        });
}

/// Full-screen UI shown while in loading state.
pub fn loading_ui(ui: &mut egui::Ui, rx: &ReceiveSet<LogMsg>) {
    let status_strings = status_strings(rx);
    if status_strings.is_empty() {
        return;
    }

    ui.centered_and_justified(|ui| {
        // TODO: not the best wait screen when loading multiple different things
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

fn welcome_ui_impl(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    rx: &ReceiveSet<LogMsg>,
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
                        .text_style(re_ui::ReUi::welcome_screen_h1()),
                )
                .wrap(false),
            );

            ui.add(
                egui::Label::new(
                    egui::RichText::new("Visualize multimodal data")
                        .text_style(re_ui::ReUi::welcome_screen_h2()),
                )
                .wrap(false),
            );

            ui.add_space(20.0);

            onboarding_content_ui(re_ui, ui, command_sender);

            for status_strings in status_strings(rx) {
                if status_strings.long_term {
                    ui.add_space(55.0);
                    ui.vertical_centered(|ui| {
                        ui.label(status_strings.status);
                        ui.label(
                            egui::RichText::new(status_strings.source)
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                }
            }
        });
    });
}

fn onboarding_content_ui(
    re_ui: &ReUi,
    ui: &mut Ui,
    command_sender: &re_viewer_context::CommandSender,
) {
    let column_spacing = 15.0;
    let stability_adjustment = 1.0; // minimize jitter with sizing and scroll bars
    let column_width = ((ui.available_width() - 2. * column_spacing) / 3.0 - stability_adjustment)
        .clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);

    let grid = egui::Grid::new("welcome_screen_grid")
        .spacing(egui::Vec2::splat(column_spacing))
        .min_col_width(column_width)
        .max_col_width(column_width);

    grid.show(ui, |ui| {
        image_banner(
            re_ui,
            ui,
            &re_ui::icons::WELCOME_SCREEN_LIVE_DATA,
            column_width,
        );
        image_banner(
            re_ui,
            ui,
            &re_ui::icons::WELCOME_SCREEN_RECORDED_DATA,
            column_width,
        );
        image_banner(
            re_ui,
            ui,
            &re_ui::icons::WELCOME_SCREEN_CONFIGURE,
            column_width,
        );

        ui.end_row();

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Connect to live data")
                    .strong()
                    .text_style(re_ui::ReUi::welcome_screen_h3()),
            );
            ui.label(
                egui::RichText::new(
                    "Use the Rerun SDK to stream data from your code to the Rerun Viewer. \
                     synchronized data from multiple processes, locally or over a network.",
                )
                .text_style(re_ui::ReUi::welcome_screen_body()),
            );
        });

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Load recorded data")
                    .strong()
                    .text_style(re_ui::ReUi::welcome_screen_h3()),
            );
            ui.label(
                egui::RichText::new(
                    "Open and visualize recorded data from previous Rerun sessions (.rrd) as well \
                    as data in formats like .gltf and .jpg.",
                )
                .text_style(re_ui::ReUi::welcome_screen_body()),
            );
        });

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Configure your views")
                    .strong()
                    .text_style(re_ui::ReUi::welcome_screen_h3()),
            );
            ui.label(
                egui::RichText::new(
                    "Add and rearrange views, and configure what data is shown and how. Configure \
                    interactively in the viewer or (coming soon) directly from code in the SDK.",
                )
                .text_style(re_ui::ReUi::welcome_screen_body()),
            );
        });

        ui.end_row();

        ui.horizontal(|ui| {
            button_centered_label(ui, "Quick start…");
            // TODO(ab): activate when C++ is ready!
            // url_large_text_button(re_ui, ui, "C++", CPP_QUICKSTART);
            url_large_text_button(re_ui, ui, "Python", PYTHON_QUICKSTART);
            url_large_text_button(re_ui, ui, "Rust", RUST_QUICKSTART);
        });

        {
            use re_ui::UICommandSender as _;
            ui.horizontal(|ui| {
                if large_text_button(ui, "Open file…").clicked() {
                    command_sender.send_ui(re_ui::UICommand::Open);
                }
                button_centered_label(ui, "Or drop a file anywhere!");
            });
        }

        #[cfg(target_arch = "wasm32")]
        ui.horizontal(|ui| {
            button_centered_label(ui, "Drop a file anywhere!");
        });

        ui.horizontal(|ui| {
            url_large_text_button(re_ui, ui, "Learn about Views", SPACE_VIEWS_HELP);
        });

        ui.end_row();
    });
}

fn button_centered_label(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) {
    ui.vertical(|ui| {
        ui.add_space(9.0);
        ui.label(label);
    });
}

fn set_large_button_style(ui: &mut egui::Ui) {
    ui.style_mut().spacing.button_padding = egui::vec2(12.0, 9.0);
    let visuals = ui.visuals_mut();
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.expansion = 0.0;
    visuals.widgets.open.expansion = 0.0;

    visuals.widgets.inactive.rounding = egui::Rounding::same(8.);
    visuals.widgets.hovered.rounding = egui::Rounding::same(8.);
    visuals.widgets.active.rounding = egui::Rounding::same(8.);

    visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;
}

fn url_large_text_button(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    url: &str,
) {
    ui.scope(|ui| {
        set_large_button_style(ui);

        let image = re_ui.icon_image(&re_ui::icons::EXTERNAL_LINK);
        let texture_id = image.texture_id(ui.ctx());

        if egui::Button::image_and_text(texture_id, ReUi::small_icon_size(), text)
            .ui(ui)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(url)
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

#[allow(dead_code)] // TODO(ab): remove if/when wasm uses one of these buttons
fn large_text_button(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.scope(|ui| {
        set_large_button_style(ui);
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
        re_smart_channel::SmartChannelSource::File { path } => {
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
