use egui::KeyboardShortcut;
use egui::os::OperatingSystem;
use smallvec::SmallVec;

use super::CommandEnvironment;

/// Interface for sending [`TableCommand`] messages.
pub trait TableCommandSender {
    fn send_table_command(&self, command: TableCommand);
}

/// A command that acts on a specific table.
///
/// Like [`super::RedapServerCommand`], these carry the table they act on, so they can be
/// used both from the command palette (acting on the currently viewed table) and from
/// other UI acting on a specific table.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableCommand {
    /// The table this command acts on.
    pub table: re_uri::TableReference,

    /// What to do with the table.
    pub kind: TableCommandKind,
}

impl TableCommand {
    pub fn text(&self) -> &'static str {
        self.kind.text()
    }

    pub fn tooltip(&self) -> &'static str {
        self.kind.tooltip()
    }
}

/// What a [`TableCommand`] does to its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum TableCommandKind {
    /// Re-query the contents (the dataframe) of the entry from the server.
    Refresh,

    /// Reset the active blueprint of the table to the default one.
    ResetBlueprint,
}

impl TableCommandKind {
    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Refresh => (
                "Refresh table",
                "Refresh the contents of the current dataset or table",
            ),

            Self::ResetBlueprint => (
                "Reset to default blueprint",
                "Clear the active blueprint of the current table and use the default blueprint instead",
            ),
        }
    }

    /// Pair this command with the currently viewed table (from `env`) to make it dispatchable.
    ///
    /// Returns `None` when no table is being viewed, or when the command does not apply to it.
    pub fn for_environment(self, env: &CommandEnvironment) -> Option<TableCommand> {
        let table = env.table.clone()?;

        // Only a Redap entry can be re-queried from its server.
        if self == Self::Refresh && !matches!(table, re_uri::TableReference::RedapEntry { .. }) {
            return None;
        }

        Some(TableCommand { table, kind: self })
    }

    /// All keyboard shortcuts, with the primary first.
    ///
    /// Note: any command with a shortcut must be paired with the viewed entry when
    /// listening for shortcuts — see [`Self::for_environment`].
    pub fn kb_shortcuts(self, os: OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
        match self {
            // This intentionally matches `RedapServerCommandKind::Refresh` — both are resolved
            // against the environment, and the table refresh wins when an entry is viewed.
            Self::Refresh => super::refresh_shortcuts(os),

            Self::ResetBlueprint => SmallVec::new(),
        }
    }

    /// Primary keyboard shortcut.
    pub fn primary_kb_shortcut(self, os: OperatingSystem) -> Option<KeyboardShortcut> {
        self.kb_shortcuts(os).first().copied()
    }

    /// The primary keyboard shortcut, nicely formatted.
    pub fn formatted_kb_shortcut(self, egui_ctx: &egui::Context) -> Option<String> {
        self.primary_kb_shortcut(egui_ctx.os())
            .map(|shortcut| egui_ctx.format_shortcut(&shortcut))
    }
}
