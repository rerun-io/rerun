use super::{large_text_button, status_strings, url_large_text_button, WelcomeScreenResponse};
use egui::{NumExt, Ui};
use re_log_types::LogMsg;
use re_smart_channel::ReceiveSet;
use re_ui::UICommandSender;

//const CPP_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/cpp";
const PYTHON_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/python";
const RUST_QUICKSTART: &str = "https://www.rerun.io/docs/getting-started/rust";
const SPACE_VIEWS_HELP: &str = "https://www.rerun.io/docs/getting-started/viewer-walkthrough";

/// Show the welcome page.
///
/// Return `true` if the user wants to switch to the example page.
pub(super) fn welcome_page_ui(
    ui: &mut egui::Ui,
    rx: &ReceiveSet<LogMsg>,
    command_sender: &re_viewer_context::CommandSender,
) -> WelcomeScreenResponse {
    ui.vertical(|ui| {
        let show_example = onboarding_content_ui(ui, command_sender);

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

        show_example
    })
    .inner
}

struct WelcomePagePanel<'a> {
    title: &'static str,
    body: &'static str,
    image: &'static re_ui::Icon,
    add_buttons: Box<dyn Fn(&mut egui::Ui) -> bool + 'a>, // returns true if example must be shown
}

fn onboarding_content_ui(
    ui: &mut Ui,
    command_sender: &re_viewer_context::CommandSender,
) -> WelcomeScreenResponse {
    // The panel data is stored in this ad hoc structure such that it can easily be iterated over
    // in chunks, to make the layout grid code simpler.
    let panels = [
        WelcomePagePanel {
            title: "Connect to live data",
            body: "Use the Rerun SDK to stream data from your code to the Rerun Viewer. \
                Visualize synchronized data from multiple processes, locally or over a network.",
            image: &re_ui::icons::WELCOME_SCREEN_LIVE_DATA,
            add_buttons: Box::new(|ui: &mut egui::Ui| {
                // TODO(ab): activate when C++ is ready!
                // url_large_text_button(ui, "C++", CPP_QUICKSTART);
                url_large_text_button(ui, "Python", PYTHON_QUICKSTART);
                url_large_text_button(ui, "Rust", RUST_QUICKSTART);

                false
            }),
        },
        WelcomePagePanel {
            title: "Load recorded data",
            body:
                "Open and visualize recorded data from previous Rerun sessions (.rrd) as well as \
                data in other formats like .gltf and .jpg. Files can be local or remote.",
            image: &re_ui::icons::WELCOME_SCREEN_RECORDED_DATA,
            add_buttons: Box::new(|ui: &mut egui::Ui| {
                if large_text_button(ui, "Open file…").clicked() {
                    command_sender.send_ui(re_ui::UICommand::Open);
                }

                false
            }),
        },
        WelcomePagePanel {
            title: "Build your views",
            body: "Add and rearrange views. Configure what data is shown and how. Design \
                interactively in the viewer or (coming soon) directly from code in the SDK.",
            image: &re_ui::icons::WELCOME_SCREEN_CONFIGURE,
            add_buttons: Box::new(|ui: &mut egui::Ui| {
                url_large_text_button(ui, "Learn about Views", SPACE_VIEWS_HELP);

                false
            }),
        },
        WelcomePagePanel {
            title: "Start with an example",
            body: "Load pre-built examples to explore what you can build with Rerun. Each example \
                comes with easy to run code so you can see how it's done.",
            image: &re_ui::icons::WELCOME_SCREEN_EXAMPLES,
            add_buttons: Box::new(|ui: &mut egui::Ui| {
                large_text_button(ui, "View Examples").clicked()
            }),
        },
    ];

    // Shrink images if needed so user can see all of the content buttons
    let max_image_height = ui.available_height() - 300.0;

    let centering_vspace = (ui.available_height() - 650.0) / 2.0;
    ui.add_space(centering_vspace.at_least(0.0));

    let panel_count = panels.len();

    const MAX_COLUMN_WIDTH: f32 = 255.0;
    const MIN_COLUMN_WIDTH: f32 = 164.0;

    let grid_spacing = egui::vec2(12.0, 16.0);

    let mut column_count = (((ui.available_width() + grid_spacing.x)
        / (MIN_COLUMN_WIDTH + grid_spacing.x))
        .floor() as usize)
        .clamp(1, panels.len());

    // we either display 4, 2, or a single column, because 3 columns is ugly with 4 panels.
    if column_count == 3 {
        column_count = 2;
    }

    let column_width = ((ui.available_width() + grid_spacing.x) / column_count as f32
        - grid_spacing.x)
        .floor()
        .at_most(MAX_COLUMN_WIDTH);

    ui.horizontal(|ui| {
        // this space is added on the left so that the grid is centered
        let centering_hspace = (ui.available_width()
            - column_count as f32 * column_width
            - (column_count - 1) as f32 * grid_spacing.x)
            / 2.0;
        ui.add_space(centering_hspace.at_least(0.0));

        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Label::new(
                    egui::RichText::new("Welcome.")
                        .strong()
                        .line_height(Some(32.0))
                        .text_style(re_ui::ReUi::welcome_screen_h1()),
                ));

                ui.add(egui::Label::new(
                    egui::RichText::new("Visualize multimodal data.")
                        .line_height(Some(32.0))
                        .text_style(re_ui::ReUi::welcome_screen_h1()),
                ));
            });

            ui.add_space(32.0);

            let grid = egui::Grid::new("welcome_screen_grid")
                .spacing(grid_spacing)
                .min_col_width(column_width)
                .max_col_width(column_width);

            grid.show(ui, |ui| {
                let mut show_example = false;

                for panels in panels.chunks(column_count) {
                    if column_count == panel_count {
                        for panel in panels {
                            image_banner(ui, panel.image, column_width, max_image_height);
                        }
                    } else {
                        for _ in panels {
                            ui.vertical(|ui| {
                                ui.add_space(20.0);
                            });
                        }
                    }

                    ui.end_row();

                    for panel in panels {
                        ui.vertical(|ui| {
                            // don't let the text get too close to the right-hand content
                            ui.set_max_width(column_width - 8.0);

                            ui.label(
                                egui::RichText::new(panel.title)
                                    .strong()
                                    .text_style(re_ui::ReUi::welcome_screen_h3()),
                            );
                            ui.label(egui::RichText::new(panel.body).line_height(Some(19.0)));
                        });
                    }

                    ui.end_row();

                    for panel in panels {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if (panel.add_buttons)(ui) {
                                show_example = true;
                            }
                        });
                    }

                    ui.end_row();
                }

                WelcomeScreenResponse {
                    go_to_example_page: show_example,
                }
            })
            .inner
        })
        .inner
    })
    .inner
}

fn image_banner(ui: &mut egui::Ui, icon: &re_ui::Icon, column_width: f32, max_image_height: f32) {
    if max_image_height < 96.0 {
        return; // skip the image if it is too small
    }

    ui.centered_and_justified(|ui| {
        ui.add(
            icon.as_image()
                .fit_to_exact_size(egui::Vec2::new(column_width, max_image_height))
                .rounding(egui::Rounding::same(8.)),
        );
    });
}
