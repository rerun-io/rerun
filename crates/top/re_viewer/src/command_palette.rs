//! The viewer's command palette: a fuzzy-searchable list of commands
//! ([`re_ui::UICommand`]s, commands acting on the active recording, and commands acting on the
//! selected Redap server), entity and component paths in the active recording, Redap servers
//! and their entries (datasets and tables) known to the viewer, and a fallback for opening any
//! URL or file path the user pastes.

use std::task::Poll;

use re_entity_db::EntityDb;
use re_log_types::{ComponentPath, EntityPath, EntryId};
use re_redap_browser::RedapServers;
use re_ui::{
    BoundCommand, CmdRow, CommandEnvironment, CommandPaletteProvider, FuzzyMatch, FuzzyQuery,
    ListedCommand, MatchGroup, MatchedCmd, SyntaxHighlighting as _,
};
use re_viewer_context::open_url::ViewerOpenUrl;

use crate::open_url_description::ViewerOpenUrlDescription;

/// Something the user can pick in the command palette.
#[derive(Clone, Debug)]
pub enum CommandPaletteAction {
    /// Run a command: a UI command, or one acting on the active recording, the selected Redap
    /// server, or the Redap entry (dataset or table) being viewed.
    Command(BoundCommand),

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
            Self::Command(command) => command.kind().tooltip(),
            Self::SelectEntityPath(_) => "Select and focus on this entity",
            Self::SelectComponentPath(_) => "Select and focus on this component",
            Self::SelectRedapServer(_) => "Select and navigate to this Redap server",
            Self::SelectRedapEntry { .. } => "Select and navigate to this entry",
            Self::OpenUrl(_) => {
                "Try to open this URL in the viewer. If the contents are already loaded, this will select them."
            }
        }
    }

    #[cfg(debug_assertions)]
    pub fn is_debug_only(&self) -> bool {
        match self {
            Self::Command(command) => command.kind().is_debug_only(),
            Self::SelectEntityPath(_)
            | Self::SelectComponentPath(_)
            | Self::SelectRedapServer(_)
            | Self::SelectRedapEntry { .. }
            | Self::OpenUrl(_) => false,
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

            re_ui::palette_commands(cmd_env)
                .into_iter()
                .filter_map(|ListedCommand { command, enabled }| {
                    match_command(
                        command.kind().text(),
                        enabled,
                        CommandPaletteAction::Command(command),
                    )
                })
                .collect()
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
            CommandPaletteAction::Command(command) => command
                .kind()
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
            CommandPaletteAction::Command(_)
            | CommandPaletteAction::SelectRedapServer(_)
            | CommandPaletteAction::SelectRedapEntry { .. }
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

        // Mark commands that only exist in debug builds:
        #[cfg(debug_assertions)]
        if matched.command.is_debug_only() {
            re_ui::debug_only::append_debug_only_badge(&mut job, ui.style());
        }

        CmdRow {
            job,
            kb_shortcut,
            tooltip: Some(matched.command.tooltip().to_owned()),
        }
    }
}
