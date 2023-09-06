use super::{button_centered_label, large_text_button, status_strings, url_large_text_button};
use egui::Ui;
use re_log_types::LogMsg;
use re_smart_channel::ReceiveSet;
use re_ui::{ReUi, UICommandSender};

const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 400.0;

//const CPP_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/cpp";
const PYTHON_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/python";
const RUST_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/rust";
const SPACE_VIEWS_HELP: &str = "https://www.rerun.io/docs/getting-started/viewer-walkthrough";

pub(super) fn welcome_page_ui(
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

        ui.horizontal(|ui| {
            if large_text_button(ui, "Open file…").clicked() {
                command_sender.send_ui(re_ui::UICommand::Open);
            }
            button_centered_label(ui, "Or drop a file anywhere!");
        });

        ui.horizontal(|ui| {
            url_large_text_button(re_ui, ui, "Learn about Views", SPACE_VIEWS_HELP);
        });

        ui.end_row();
    });
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
