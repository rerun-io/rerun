//! `cargo r -p re_ui --example cmd_palette_example`

use re_log_types::EntityPath;
use re_ui::{
    CmdRow, CommandPalette, CommandPaletteProvider, FuzzyMatch, FuzzyQuery, MatchGroup, MatchedCmd,
    SyntaxHighlighting as _, UICommand, UiExt as _,
};
use url::Url;

fn main() -> eframe::Result {
    let mut state = State::default();
    let set_uo = std::sync::Once::new();

    let options = eframe::NativeOptions::default();
    eframe::run_ui_native("My egui App", options, move |ui, _frame| {
        set_uo.call_once(|| {
            re_ui::apply_style_and_install_loaders(ui);
        });

        state.ui(ui);
    })
}

#[expect(dead_code, reason = "This is an example")]
#[derive(Debug)]
enum Command {
    #[expect(clippy::enum_variant_names, reason = "This is an example")]
    UiCommand(UICommand),

    SelectEntityPath(String),

    OpenUrl(Url),
}

impl Command {
    fn tooltip(&self) -> &'static str {
        match self {
            Self::UiCommand(command) => command.tooltip(),
            Self::SelectEntityPath(_) => "Select and focus on this entity",
            Self::OpenUrl(_) => {
                "Open this URL in the viewer. If the contents are already loaded, this will select them."
            }
        }
    }

    fn formatted_kb_shortcut(&self, egui_ctx: &egui::Context) -> Option<String> {
        match self {
            Self::UiCommand(command) => command.formatted_kb_shortcut(egui_ctx),
            Self::SelectEntityPath(_) | Self::OpenUrl(_) => None,
        }
    }
}

struct RecordingInfo {
    entities: Vec<&'static str>,
}

struct CommandProvider {
    /// Info about the open recording, if any
    rec: Option<RecordingInfo>,
}

impl CommandPaletteProvider<Command> for CommandProvider {
    fn initial_hint_ui(&mut self, ui: &mut egui::Ui) {
        if self.rec.is_some() {
            ui.weak("Find a command, search for an entity, or enter an URL to open");
        } else {
            ui.weak("Find a command or enter an URL to open");
        }
        ui.add_space(4.0);
    }

    fn all_matching(&mut self, query: &FuzzyQuery) -> Vec<MatchGroup<Command>> {
        use strum::IntoEnumIterator as _;

        let ui_cmd_group = if query.raw_query().starts_with('/') {
            vec![] // The user is looking for an entity
        } else {
            UICommand::iter()
                .filter_map(|command| {
                    let enabled = true;
                    let target_text = command.text();
                    let command = Command::UiCommand(command);

                    if query.is_empty() {
                        // Nothing entered yet: show all commands:
                        Some(MatchedCmd {
                            command,
                            fuzzy_match: FuzzyMatch::lowest(target_text.to_owned()),
                            enabled,
                        })
                    } else {
                        query
                            .try_match(target_text.to_owned())
                            .map(|fuzzy_match| MatchedCmd {
                                command,
                                fuzzy_match,
                                enabled,
                            })
                    }
                })
                .collect()
        };

        let entity_group = if query.is_empty() {
            vec![] // Nothing entered yet: only show commands, no entities
        } else if let Some(rec) = &self.rec {
            rec.entities
                .iter()
                .filter_map(|&entity_path| {
                    query
                        .try_match(entity_path.to_owned())
                        .map(|fuzzy_match| MatchedCmd {
                            command: Command::SelectEntityPath(entity_path.to_owned()),
                            fuzzy_match,
                            enabled: true,
                        })
                })
                .collect()
        } else {
            vec![] // no recording open
        };

        let url_group = if let Ok(url) = query.raw_query().trim().parse::<Url>() {
            // The user entered a URL. Let's open it!
            vec![MatchedCmd {
                fuzzy_match: FuzzyMatch::highest(url.to_string()),
                command: Command::OpenUrl(url),
                enabled: true,
            }]
        } else {
            vec![]
        };

        vec![ui_cmd_group, entity_group, url_group]
    }

    fn cmd_row(&self, ui: &egui::Ui, matched: &MatchedCmd<Command>, selected: bool) -> CmdRow {
        let kb_shortcut = matched
            .command
            .formatted_kb_shortcut(ui.ctx())
            .unwrap_or_default();

        let text_color = if !matched.enabled {
            ui.visuals().weak_text_color()
        } else if selected {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };

        let job = if let Command::SelectEntityPath(ent_path) = &matched.command {
            let mut job = EntityPath::parse_forgiving(ent_path).syntax_highlighted(ui.style());
            if selected {
                // The syntax colors clash with the selection background, so recolor the
                // whole path to the selection text color (keeping the font/size).
                for section in &mut job.sections {
                    section.format.color = text_color;
                }
            }
            job
        } else {
            egui::text::LayoutJob::simple(
                matched.fuzzy_match.target().to_owned(),
                egui::TextStyle::Button.resolve(ui.style()),
                text_color,
                f32::INFINITY,
            )
        };

        let job = if matched.enabled {
            // Only highlight the matched characters on available commands;
            // unavailable ones stay uniformly grayed out.
            matched
                .fuzzy_match
                .highlight_matching_text(ui.style(), &job, selected)
        } else {
            job
        };

        CmdRow {
            job,
            kb_shortcut,
            tooltip: Some(matched.command.tooltip().to_owned()),
        }
    }
}

struct State {
    cmd_palette: CommandPalette,
    has_recording: bool,
    latest_command: Option<Command>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            cmd_palette: Default::default(),
            has_recording: true,
            latest_command: Default::default(),
        }
    }
}

impl State {
    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Press cmd-K");

            ui.re_checkbox(&mut self.has_recording, "Pretend to have a recording");
        });

        let mut cmd_provider = CommandProvider {
            rec: self.has_recording.then(|| RecordingInfo {
                // Fake-match a bunch of entities.
                entities: vec![
                    "/camera/pinhole",
                    "/camera/rgb",
                    "/world",
                    "/world/car",
                    "/world/car/front_camera",
                    "/world/car/imu",
                    "/world/road",
                ],
            }),
        };

        if let Some(command) = self.cmd_palette.show(ui.ctx(), &mut cmd_provider) {
            self.on_command(ui, command);
        }

        if let Some(ui_cmd) = re_ui::UICommand::listen_for_kb_shortcut(ui) {
            self.on_command(ui, Command::UiCommand(ui_cmd));
        }
    }

    fn on_command(&mut self, ui: &egui::Ui, command: Command) {
        match command {
            Command::UiCommand(ui_cmd) => match ui_cmd {
                UICommand::ToggleCommandPalette => self.cmd_palette.toggle(),
                UICommand::ZoomIn => {
                    let mut zoom_factor = ui.zoom_factor();
                    zoom_factor += 0.1;
                    ui.set_zoom_factor(zoom_factor);
                }
                UICommand::ZoomOut => {
                    let mut zoom_factor = ui.zoom_factor();
                    zoom_factor -= 0.1;
                    ui.set_zoom_factor(zoom_factor);
                }
                UICommand::ZoomReset => {
                    ui.set_zoom_factor(1.0);
                }
                _ => {}
            },
            Command::SelectEntityPath(_) | Command::OpenUrl(_) => {
                //
            }
        }

        self.latest_command = Some(command);
    }
}
