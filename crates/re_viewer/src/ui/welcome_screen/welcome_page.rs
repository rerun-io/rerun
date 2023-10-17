use super::{large_text_button, status_strings, url_large_text_button, WelcomeScreenResponse};
use egui::{NumExt, Ui};
use re_data_store::StoreDb;
use re_log_types::{
    DataRow, EntityPath, LogMsg, RowId, StoreId, StoreInfo, StoreKind, StoreSource, Time, TimePoint,
};
use re_smart_channel::ReceiveSet;
use re_ui::UICommandSender;
use re_viewer_context::{SystemCommand, SystemCommandSender};
use std::collections::HashMap;

const SPACE_VIEWS_HELP: &str = "https://www.rerun.io/docs/getting-started/viewer-walkthrough";

const HOW_DOES_IT_WORK: &str = include_str!("../../../data/quick_start_guides/how_does_it_work.md");

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
                //TODO(#3870): enable with C++ guides are completed
                #[allow(clippy::collapsible_if)]
                if false {
                    if large_text_button(ui, "C++").clicked() {
                        open_quick_start(
                            command_sender,
                            include_str!("../../../data/quick_start_guides/cpp_native.md"),
                            [
                                (
                                    "EXAMPLE_CODE",
                                    include_str!(
                                        "../../../data/quick_start_guides/quick_start_connect.cpp"
                                    ),
                                ),
                                ("HOW_DOES_IT_WORK", HOW_DOES_IT_WORK),
                                ("SAFARI_WARNING", safari_warning()),
                            ]
                            .into(),
                            "C++ Quick Start",
                            "cpp_quick_start",
                        );
                    }
                }
                if large_text_button(ui, "Python").clicked() {
                    open_quick_start(
                        command_sender,
                        include_str!("../../../data/quick_start_guides/python_native.md"),
                        [
                            (
                                "EXAMPLE_CODE",
                                include_str!(
                                    "../../../data/quick_start_guides/quick_start_connect.py"
                                ),
                            ),
                            ("HOW_DOES_IT_WORK", HOW_DOES_IT_WORK),
                            ("SAFARI_WARNING", safari_warning()),
                        ]
                        .into(),
                        "Python Quick Start",
                        "python_quick_start",
                    );
                }
                if large_text_button(ui, "Rust").clicked() {
                    open_quick_start(
                        command_sender,
                        include_str!("../../../data/quick_start_guides/rust_native.md"),
                        [
                            (
                                "EXAMPLE_CODE",
                                include_str!(
                                    "../../../data/quick_start_guides/quick_start_connect.rs"
                                ),
                            ),
                            ("HOW_DOES_IT_WORK", HOW_DOES_IT_WORK),
                            ("SAFARI_WARNING", safari_warning()),
                        ]
                        .into(),
                        "Rust Quick Start",
                        "rust_quick_start",
                    );
                }

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

    // Shrink images if needed so user can see all the content buttons
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

/// Open a Quick Start recording
///
/// The markdown content may contain placeholders in the form of `${NAME}`. These will be replaced
/// with the corresponding value from the `placeholder_content` hash map.
fn open_quick_start(
    command_sender: &re_viewer_context::CommandSender,
    markdown: &str,
    placeholder_content: HashMap<&'static str, &'static str>,
    app_id: &str,
    entity_path: &str,
) {
    let mut markdown = markdown.to_owned();

    for (key, value) in placeholder_content {
        markdown = markdown.replace(format!("${{{key}}}").as_str(), value);
    }

    let res = open_markdown_recording(command_sender, markdown.as_str(), app_id, entity_path);
    if let Err(err) = res {
        re_log::error!("Failed to load quick start: {}", err);
    }
}

fn open_markdown_recording(
    command_sender: &re_viewer_context::CommandSender,
    markdown: &str,
    app_id: &str,
    entity_path: &str,
) -> anyhow::Result<()> {
    let text_doc = re_types::archetypes::TextDocument::new(markdown)
        .with_media_type(re_types::components::MediaType::markdown());

    let row = DataRow::from_archetype(
        RowId::random(),
        TimePoint::timeless(),
        EntityPath::from(entity_path),
        &text_doc,
    )?;

    let store_info = StoreInfo {
        application_id: app_id.into(),
        store_id: StoreId::random(StoreKind::Recording),
        is_official_example: true,
        started: Time::now(),
        store_source: StoreSource::Viewer,
        store_kind: StoreKind::Recording,
    };

    let store_db = StoreDb::from_info_and_rows(store_info, [row])?;
    command_sender.send_system(SystemCommand::LoadStoreDb(store_db));

    Ok(())
}

/// The User-Agent of the user's browser.
fn user_agent() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return eframe::web::user_agent();

    #[cfg(not(target_arch = "wasm32"))]
    None
}

/// Are we running on Safari?
fn safari_warning() -> &'static str {
    // Note that this implementation is very naive and might return false positives. This is ok for
    // the purpose of displaying a "can't copy" warning in the Quick Start guide, but this detection
    // is likely not suitable for pretty much anything else.
    //
    // See this page for more information on User Agent sniffing (and why/how to avoid it):
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Browser_detection_using_the_user_agent

    let is_safari = user_agent().is_some_and(|user_agent| {
        user_agent.contains("Safari")
            && !user_agent.contains("Chrome")
            && !user_agent.contains("Chromium")
    });

    if is_safari {
        "**Note**: This browser appears to be Safari. If you are unable to copy the code, please \
        try a different browser (see [this issue](https://github.com/emilk/egui/issues/3480))."
    } else {
        ""
    }
}
