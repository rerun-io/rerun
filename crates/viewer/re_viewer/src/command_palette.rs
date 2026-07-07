//! The viewer's command palette: a fuzzy-searchable list of commands
//! ([`UICommand`]s, commands acting on the active recording, and commands acting on the
//! selected Redap server), entity and component paths in the active recording, Redap servers
//! and their entries (datasets and tables) known to the viewer, and a fallback for opening any
//! URL or file path the user pastes.

use std::task::Poll;

use re_entity_db::EntityDb;
use re_log_types::{ComponentPath, EntityPath, EntryId};
use re_redap_browser::RedapServers;
use re_ui::{
    CmdRow, CommandEnvironment, CommandPaletteProvider, FuzzyMatch, FuzzyQuery, MatchGroup,
    MatchedCmd, RecordingCommand, RecordingCommandKind, RedapServerCommand,
    SyntaxHighlighting as _, TableCommand, TableCommandKind, UICommand,
};
use re_viewer_context::open_url::ViewerOpenUrl;

use crate::open_url_description::ViewerOpenUrlDescription;

/// Something the user can pick in the command palette.
#[derive(Clone, Debug)]
pub enum CommandPaletteAction {
    /// Run a UI command.
    UiCommand(UICommand),

    /// Run a command on a specific recording.
    RecordingCommand(RecordingCommand),

    /// Run a command on the currently selected Redap server.
    RedapServerCommand(RedapServerCommand),

    /// Select and focus an entity in the active recording.
    SelectEntityPath(EntityPath),

    /// Select and focus a component of an entity in the active recording.
    SelectComponentPath(ComponentPath),

    /// Select a Redap server known to the viewer.
    SelectRedapServer(re_uri::Origin),

    /// Select an entry (dataset or table) on a Redap server known to the viewer.
    SelectRedapEntry {
        origin: re_uri::Origin,
        entry_id: EntryId,

        /// The viewer is connected to more than one server, so the row should also show the
        /// server this entry belongs to.
        show_server: bool,
    },

    /// Run a command on the Redap entry (dataset or table) currently being viewed.
    TableCommand(TableCommand),

    /// Open a URL (or file path).
    ///
    /// URL opening is the fallback for the command palette and needs some special treatment since
    /// ui commands usually don't have arbitrary state. We keep the raw query string and let the
    /// handler re-parse it, so this also covers file paths and schemeless URLs.
    OpenUrl(String),
}

impl CommandPaletteAction {
    fn tooltip(&self) -> &'static str {
        match self {
            Self::UiCommand(command) => command.tooltip(),
            Self::RecordingCommand(command) => command.kind.tooltip(),
            Self::RedapServerCommand(command) => command.tooltip(),
            Self::SelectEntityPath(_) => "Select and focus on this entity",
            Self::SelectComponentPath(_) => "Select and focus on this component",
            Self::SelectRedapServer(_) => "Select and navigate to this Redap server",
            Self::SelectRedapEntry { .. } => "Select and navigate to this entry",
            Self::TableCommand(command) => command.tooltip(),
            Self::OpenUrl(_) => {
                "Try to open this URL in the viewer. If the contents are already loaded, this will select them."
            }
        }
    }
}

/// Feeds the viewer's commands into the [`re_ui::CommandPalette`].
pub struct CommandPaletteProviderImpl<'a> {
    /// The active recording, if any. Provides entity-path completion.
    pub recording: Option<&'a EntityDb>,

    /// All Redap servers known to the viewer. Provides server- and entry-name completion.
    pub redap_servers: &'a RedapServers,

    /// Determines which commands are currently available.
    pub cmd_env: CommandEnvironment,
}

impl CommandPaletteProvider<CommandPaletteAction> for CommandPaletteProviderImpl<'_> {
    fn initial_hint_ui(&mut self, ui: &mut egui::Ui) {
        if self.recording.is_some() {
            ui.weak(
                "Find a command, search for an entity, dataset or table, or enter a URL to open",
            );
        } else {
            ui.weak(
                "Find a command, search for a server, dataset or table, or enter a URL to open",
            );
        }
        ui.add_space(4.0);
    }

    fn all_matching(&mut self, query: &FuzzyQuery) -> Vec<MatchGroup<CommandPaletteAction>> {
        re_tracing::profile_function!();
        use strum::IntoEnumIterator as _;

        let ui_cmd_group = if query.raw_query().starts_with('/') {
            vec![] // The user is looking for an entity path.
        } else {
            let cmd_env = &self.cmd_env;

            // Helper to match a command against the query:
            let match_command = |target_text: &str, enabled: bool, command| {
                if query.is_empty() {
                    // Nothing entered yet: show all commands.
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
            };

            let mut matches: Vec<_> = UICommand::iter()
                .filter_map(|command| {
                    match_command(
                        command.text(),
                        true,
                        CommandPaletteAction::UiCommand(command),
                    )
                })
                .collect();

            // Commands acting on the active recording, if any:
            if let Some(recording_id) = &cmd_env.recording {
                for command in RecordingCommand::all_for_recording(recording_id) {
                    // `PlaybackSpeed` is a chord (type e.g. `5` then `0`), not a single
                    // action — as a palette entry it would just reset the speed to 1x.
                    if matches!(command.kind, RecordingCommandKind::PlaybackSpeed(_)) {
                        continue;
                    }
                    matches.extend(match_command(
                        command.kind.text(),
                        true,
                        CommandPaletteAction::RecordingCommand(command),
                    ));
                }
            }

            // Commands acting on the selected Redap server, if any:
            if let Some(origin) = &cmd_env.redap_server {
                for command in RedapServerCommand::all_for_server(origin) {
                    let enabled =
                        !command.requires_editable_server() || cmd_env.has_editable_redap_server;
                    matches.extend(match_command(
                        command.text(),
                        enabled,
                        CommandPaletteAction::RedapServerCommand(command),
                    ));
                }
            }

            // Commands acting on the Redap entry currently being viewed, if any:
            for kind in TableCommandKind::iter() {
                if let Some(command) = kind.for_environment(cmd_env) {
                    matches.extend(match_command(
                        command.text(),
                        true,
                        CommandPaletteAction::TableCommand(command),
                    ));
                }
            }

            matches
        };

        let entity_group = if query.is_empty() {
            vec![] // Nothing entered yet: only show commands, no entities.
        } else if let Some(recording) = self.recording {
            let engine = recording.storage_engine();
            let schema = engine.store().schema();

            // We fuzzy-match against the same (unescaped, syntax-highlight) text that
            // `cmd_row` renders, so `FuzzyMatch::highlight_matching_text` lines up.
            // The style doesn't affect the resulting text, so a default one is fine.
            let style = egui::Style::default();

            let mut matches = Vec::new();
            for entity_path in recording.sorted_entity_paths() {
                if let Some(fuzzy_match) =
                    query.try_match(entity_path.syntax_highlighted(&style).text)
                {
                    matches.push(MatchedCmd {
                        command: CommandPaletteAction::SelectEntityPath(entity_path.clone()),
                        fuzzy_match,
                        enabled: true,
                    });
                }

                // Also offer each component ever logged to this entity:
                if let Some(components) = schema.all_components_for_entity(entity_path) {
                    for &component in components {
                        let component_path = ComponentPath::new(entity_path.clone(), component);
                        if let Some(fuzzy_match) =
                            query.try_match(component_path.syntax_highlighted(&style).text)
                        {
                            matches.push(MatchedCmd {
                                command: CommandPaletteAction::SelectComponentPath(component_path),
                                fuzzy_match,
                                enabled: true,
                            });
                        }
                    }
                }
            }
            matches
        } else {
            vec![]
        };

        // Redap servers and entries (datasets and tables) known to the viewer.
        // Entries are grouped per server; if a server is selected, only its entries are offered.
        // Skip when the user is clearly typing an entity path (leading `/`).
        let (server_group, entry_groups) = if query.is_empty() || query.raw_query().starts_with('/')
        {
            (vec![], vec![])
        } else {
            let selected_server = self.cmd_env.redap_server.as_ref();

            // When entries from more than one server can show up,
            // show which server each entry belongs to.
            let show_server =
                selected_server.is_none() && 1 < self.redap_servers.iter_servers().count();

            let mut server_matches = Vec::new();
            let mut entry_groups: Vec<MatchGroup<CommandPaletteAction>> = Vec::new();
            for server in self.redap_servers.iter_servers() {
                let origin = server.origin();
                if let Some(fuzzy_match) = query.try_match(origin.host.to_string()) {
                    server_matches.push(MatchedCmd {
                        command: CommandPaletteAction::SelectRedapServer(origin.clone()),
                        fuzzy_match,
                        enabled: true,
                    });
                }

                // If a server is selected, only offer entries from that server:
                if selected_server.is_some_and(|selected| selected != origin) {
                    continue;
                }

                if let Poll::Ready(Ok(entries)) = server.entries().state() {
                    // Offer every entry (datasets and tables) by name.
                    let mut entries: Vec<_> = entries.values().collect();
                    entries.sort_by_key(|entry| entry.id());

                    let mut group = Vec::new();
                    for entry in entries {
                        if let Some(fuzzy_match) = query.try_match(entry.name().to_string()) {
                            group.push(MatchedCmd {
                                command: CommandPaletteAction::SelectRedapEntry {
                                    origin: origin.clone(),
                                    entry_id: entry.id(),
                                    show_server,
                                },
                                fuzzy_match,
                                enabled: true,
                            });
                        }
                    }
                    if !group.is_empty() {
                        entry_groups.push(group);
                    }
                }
            }

            (server_matches, entry_groups)
        };

        let raw_url = query.raw_query().trim();
        let url_group = if let Ok(open_url) = ViewerOpenUrl::parse_with_options(
            raw_url,
            &re_data_source::FromUriOptions {
                accept_extensionless_http: true,
                ..Default::default()
            },
        ) {
            // The user entered something openable (URL, file path, …). Offer to open it!
            let command_text = format!("Open {}", ViewerOpenUrlDescription::from_url(&open_url));
            vec![MatchedCmd {
                fuzzy_match: FuzzyMatch::highest(command_text),
                command: CommandPaletteAction::OpenUrl(raw_url.to_owned()),
                enabled: true,
            }]
        } else {
            vec![]
        };

        itertools::chain!(
            [ui_cmd_group, entity_group, server_group],
            entry_groups,
            [url_group],
        )
        .collect()
    }

    fn cmd_row(
        &self,
        ui: &egui::Ui,
        matched: &MatchedCmd<CommandPaletteAction>,
        selected: bool,
    ) -> CmdRow {
        let kb_shortcut = match &matched.command {
            CommandPaletteAction::UiCommand(command) => {
                command.formatted_kb_shortcut(ui.ctx()).unwrap_or_default()
            }
            CommandPaletteAction::RecordingCommand(command) => command
                .kind
                .formatted_kb_shortcut(ui.ctx())
                .unwrap_or_default(),
            CommandPaletteAction::RedapServerCommand(command) => command
                .kind
                .formatted_kb_shortcut(ui.ctx())
                .unwrap_or_default(),
            CommandPaletteAction::TableCommand(command) => command
                .kind
                .formatted_kb_shortcut(ui.ctx())
                .unwrap_or_default(),
            CommandPaletteAction::SelectEntityPath(_)
            | CommandPaletteAction::SelectComponentPath(_)
            | CommandPaletteAction::SelectRedapServer(_)
            | CommandPaletteAction::SelectRedapEntry { .. }
            | CommandPaletteAction::OpenUrl(_) => String::new(),
        };

        let text_color = if !matched.enabled {
            ui.visuals().weak_text_color()
        } else if selected {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };

        // On the selected row the syntax colors clash with the selection background,
        // so recolor the whole (syntax-highlighted) path to the selection text color.
        // We keep the syntax-highlighted job either way, so the font/size stays the same.
        let recolor_if_selected = |mut job: egui::text::LayoutJob| {
            if selected {
                for section in &mut job.sections {
                    section.format.color = text_color;
                }
            }
            job
        };

        let job = match &matched.command {
            CommandPaletteAction::SelectEntityPath(entity_path) => {
                recolor_if_selected(entity_path.syntax_highlighted(ui.style()))
            }
            CommandPaletteAction::SelectComponentPath(component_path) => {
                recolor_if_selected(component_path.syntax_highlighted(ui.style()))
            }
            CommandPaletteAction::UiCommand(_)
            | CommandPaletteAction::RecordingCommand(_)
            | CommandPaletteAction::RedapServerCommand(_)
            | CommandPaletteAction::SelectRedapServer(_)
            | CommandPaletteAction::SelectRedapEntry { .. }
            | CommandPaletteAction::TableCommand(_)
            | CommandPaletteAction::OpenUrl(_) => egui::text::LayoutJob::simple(
                matched.fuzzy_match.target().to_owned(),
                egui::TextStyle::Button.resolve(ui.style()),
                text_color,
                f32::INFINITY,
            ),
        };

        let mut job = if matched.enabled {
            // Only highlight the matched characters on available commands;
            // unavailable ones stay uniformly grayed out.
            // Otherwise the user may confusingly think the underlined command is the one that will be executed when they hit enter.
            matched
                .fuzzy_match
                .highlight_matching_text(ui.style(), &job, selected)
        } else {
            job
        };

        // When connected to multiple servers, append the entry's server in weak text so it
        // doesn't distract from (or fuzzy-match against) the entry name.
        if let CommandPaletteAction::SelectRedapEntry {
            origin,
            show_server: true,
            ..
        } = &matched.command
        {
            job.append(
                &format!("  {}", origin.host),
                0.0,
                egui::TextFormat::simple(
                    egui::TextStyle::Button.resolve(ui.style()),
                    if selected {
                        text_color
                    } else {
                        ui.visuals().weak_text_color()
                    },
                ),
            );
        }

        CmdRow {
            job,
            kb_shortcut,
            tooltip: Some(matched.command.tooltip().to_owned()),
        }
    }
}
