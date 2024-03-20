use super::{large_text_button, url_large_text_button};
use egui::{NumExt, Ui};
use re_entity_db::EntityDb;
use re_log_types::{
    DataRow, EntityPath, LogMsg, RowId, StoreId, StoreInfo, StoreKind, StoreSource, Time, TimePoint,
};
use re_smart_channel::ReceiveSet;
use re_ui::UICommandSender;
use re_viewer_context::CommandSender;
use re_viewer_context::{SystemCommand, SystemCommandSender};

const SPACE_VIEWS_HELP: &str = "https://www.rerun.io/docs/getting-started/viewer-walkthrough";

const CPP_CONNECT_MARKDOWN: &str = include_str!("../../../data/quick_start_guides/cpp_connect.md");
const CPP_SPAWN_MARKDOWN: &str = include_str!("../../../data/quick_start_guides/cpp_spawn.md");
const PYTHON_CONNECT_MARKDOWN: &str =
    include_str!("../../../data/quick_start_guides/python_connect.md");
const PYTHON_SPAWN_MARKDOWN: &str =
    include_str!("../../../data/quick_start_guides/python_spawn.md");
const RUST_CONNECT_MARKDOWN: &str =
    include_str!("../../../data/quick_start_guides/rust_connect.md");
const RUST_SPAWN_MARKDOWN: &str = include_str!("../../../data/quick_start_guides/rust_spawn.md");
const HOW_DOES_IT_WORK_MARKDOWN: &str =
    include_str!("../../../data/quick_start_guides/how_does_it_work.md");

const CPP_CONNECT_SNIPPET: &str =
    include_str!("../../../data/quick_start_guides/quick_start_connect.cpp");
const CPP_SPAWN_SNIPPET: &str =
    include_str!("../../../data/quick_start_guides/quick_start_spawn.cpp");
const PYTHON_CONNECT_SNIPPET: &str =
    include_str!("../../../data/quick_start_guides/quick_start_connect.py");
const PYTHON_SPAWN_SNIPPET: &str =
    include_str!("../../../data/quick_start_guides/quick_start_spawn.py");
const RUST_CONNECT_SNIPPET: &str =
    include_str!("../../../data/quick_start_guides/quick_start_connect.rs");
const RUST_SPAWN_SNIPPET: &str =
    include_str!("../../../data/quick_start_guides/quick_start_spawn.rs");

struct Placeholder<'a> {
    key: &'a str,
    value: &'a str,
}

const PLACEHOLDERS: &[Placeholder<'_>] = &[
    Placeholder {
        key: "HOW_DOES_IT_WORK",
        value: HOW_DOES_IT_WORK_MARKDOWN,
    },
    Placeholder {
        key: "EXAMPLE_CODE_CPP_CONNECT",
        value: CPP_CONNECT_SNIPPET,
    },
    Placeholder {
        key: "EXAMPLE_CODE_CPP_SPAWN",
        value: CPP_SPAWN_SNIPPET,
    },
    Placeholder {
        key: "EXAMPLE_CODE_PYTHON_CONNECT",
        value: PYTHON_CONNECT_SNIPPET,
    },
    Placeholder {
        key: "EXAMPLE_CODE_PYTHON_SPAWN",
        value: PYTHON_SPAWN_SNIPPET,
    },
    Placeholder {
        key: "EXAMPLE_CODE_RUST_CONNECT",
        value: RUST_CONNECT_SNIPPET,
    },
    Placeholder {
        key: "EXAMPLE_CODE_RUST_SPAWN",
        value: RUST_SPAWN_SNIPPET,
    },
];

struct QuickStartEntry<'a> {
    entity_path: &'a str,
    markdown: &'a str,
}

const QUICK_START_ENTRIES_CONNECT: &[QuickStartEntry<'_>] = &[
    QuickStartEntry {
        entity_path: "quick_start/cpp",
        markdown: CPP_CONNECT_MARKDOWN,
    },
    QuickStartEntry {
        entity_path: "quick_start/python",
        markdown: PYTHON_CONNECT_MARKDOWN,
    },
    QuickStartEntry {
        entity_path: "quick_start/rust",
        markdown: RUST_CONNECT_MARKDOWN,
    },
];

const QUICK_START_ENTRIES_SPAWN: &[QuickStartEntry<'_>] = &[
    QuickStartEntry {
        entity_path: "quick_start/cpp",
        markdown: CPP_SPAWN_MARKDOWN,
    },
    QuickStartEntry {
        entity_path: "quick_start/python",
        markdown: PYTHON_SPAWN_MARKDOWN,
    },
    QuickStartEntry {
        entity_path: "quick_start/rust",
        markdown: RUST_SPAWN_MARKDOWN,
    },
];

/// Show the welcome section.
pub(super) fn welcome_section_ui(
    ui: &mut egui::Ui,
    rx: &ReceiveSet<LogMsg>,
    command_sender: &re_viewer_context::CommandSender,
) {
    ui.vertical(|ui| {
        let accepts_connections = rx.accepts_tcp_connections();
        onboarding_content_ui(ui, command_sender, accepts_connections);
    });
}

struct Panel {
    title: &'static str,
    body: &'static str,
    image: &'static re_ui::Icon,
    add_buttons: PanelButtonsCallback,
}

type PanelButtonsCallback = Box<dyn Fn(&mut egui::Ui, &CommandSender) + 'static>;

fn onboarding_content_ui(ui: &mut Ui, command_sender: &CommandSender, accepts_connections: bool) {
    // The panel data is stored in this ad hoc structure such that it can easily be iterated over
    // in chunks, to make the layout grid code simpler.
    let panels = [
        Panel {
            title: "Connect to live data",
            body: "Use the Rerun SDK to stream data from your code to the Rerun Viewer. \
                Visualize synchronized data from multiple processes, locally or over a network.",
            image: &re_ui::icons::WELCOME_SCREEN_LIVE_DATA,
            add_buttons: Box::new(move |ui, command_sender| {
                if large_text_button(ui, "Quick start").clicked() {
                    let entries = if accepts_connections {
                        QUICK_START_ENTRIES_CONNECT
                    } else {
                        QUICK_START_ENTRIES_SPAWN
                    };

                    if let Err(err) = open_quick_start(command_sender, entries, PLACEHOLDERS) {
                        re_log::error!("Failed to load quick start: {}", err);
                    }
                }
            }),
        },
        Panel {
            title: "Load recorded data",
            body:
                "Open and visualize recorded data from previous Rerun sessions (.rrd) as well as \
                data in other formats like .gltf and .jpg. Files can be local or remote.",
            image: &re_ui::icons::WELCOME_SCREEN_RECORDED_DATA,
            add_buttons: Box::new(|ui, command_sender| {
                if large_text_button(ui, "Open file…").clicked() {
                    command_sender.send_ui(re_ui::UICommand::Open);
                }
            }),
        },
        Panel {
            title: "Build your views",
            body: "Add and rearrange views. Configure what data is shown and how. Design \
                interactively in the viewer or (coming soon) directly from code in the SDK.",
            image: &re_ui::icons::WELCOME_SCREEN_CONFIGURE,
            add_buttons: Box::new(|ui, _| {
                url_large_text_button(ui, "Learn about views", SPACE_VIEWS_HELP);
            }),
        },
    ];

    // Shrink images if needed so user can see all the content buttons
    let max_image_height = ui.available_height() - 300.0;

    let panel_count = panels.len();

    const MAX_COLUMN_WIDTH: f32 = 280.0;
    const MIN_COLUMN_WIDTH: f32 = 164.0;

    let grid_spacing = egui::vec2(12.0, 16.0);

    let column_count = (((ui.available_width() + grid_spacing.x)
        / (MIN_COLUMN_WIDTH + grid_spacing.x))
        .floor() as usize)
        .clamp(1, panels.len());

    let column_width = ((ui.available_width() + grid_spacing.x) / column_count as f32
        - grid_spacing.x)
        .floor()
        .at_most(MAX_COLUMN_WIDTH);

    ui.horizontal(|ui| {
        ui.vertical_centered(|ui| {
            ui.add(egui::Label::new(
                egui::RichText::new("Welcome")
                    .strong()
                    .line_height(Some(32.0))
                    .text_style(re_ui::ReUi::welcome_screen_h1()),
            ))
        });
    });
    ui.end_row();

    ui.horizontal(|ui| {
        // this space is added on the left so that the grid is centered
        let centering_hspace = (ui.available_width()
            - column_count as f32 * column_width
            - (column_count - 1) as f32 * grid_spacing.x)
            / 2.0;
        ui.add_space(centering_hspace.at_least(0.0));

        ui.vertical(|ui| {
            ui.add_space(32.0);

            let grid = egui::Grid::new("welcome_section_grid")
                .spacing(grid_spacing)
                .min_col_width(column_width)
                .max_col_width(column_width);

            grid.show(ui, |ui| {
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
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                (panel.add_buttons)(ui, command_sender);
                            });
                        });
                    }

                    ui.end_row();
                }
            });
        });
    });
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
    entries: &[QuickStartEntry<'_>],
    placeholders: &[Placeholder<'_>],
) -> anyhow::Result<()> {
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut markdown = entry.markdown.to_owned();
        for Placeholder { key, value } in placeholders {
            markdown = markdown.replace(&format!("${{{key}}}"), value);
        }

        let text_doc = re_types::archetypes::TextDocument::new(markdown)
            .with_media_type(re_types::components::MediaType::markdown());

        let row = DataRow::from_archetype(
            RowId::new(),
            TimePoint::timeless(),
            EntityPath::from(entry.entity_path),
            &text_doc,
        )?;
        rows.push(row);
    }

    let store_info = StoreInfo {
        application_id: "Quick start".into(),
        store_id: StoreId::random(StoreKind::Recording),
        is_official_example: true,
        started: Time::now(),
        store_source: StoreSource::Viewer,
        store_kind: StoreKind::Recording,
    };

    let entity_db = EntityDb::from_info_and_rows(store_info, rows)?;
    command_sender.send_system(SystemCommand::LoadStoreDb(entity_db));

    Ok(())
}
