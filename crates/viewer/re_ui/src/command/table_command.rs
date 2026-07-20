use egui::KeyboardShortcut;
use egui::os::OperatingSystem;
use smallvec::SmallVec;

use super::CommandEnvironment;

/// Interface for sending [`TableCommand`] messages.
pub trait TableCommandSender {
    fn send_table_command(&self, command: TableCommand);
}

/// A command that acts on a specific table-like Redap entry (dataset or table).
///
/// Like [`super::RedapServerCommand`], these carry the entry they act on, so they can be
/// used both from the command palette (acting on the currently viewed entry) and from
/// other UI acting on a specific entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableCommand {
    /// The server the entry lives on.
    pub origin: re_uri::Origin,

    /// The entry (dataset or table) this command acts on.
    pub entry_id: re_log_types::EntryId,

    /// What to do with the entry.
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

/// What a [`TableCommand`] does to its entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum TableCommandKind {
    /// Re-query the contents (the dataframe) of the entry from the server.
    Refresh,
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
        }
    }

    /// Pair this command with the currently viewed entry (from `env`) to make it dispatchable.
    ///
    /// Returns `None` when no entry is being viewed.
    pub fn for_environment(self, env: &CommandEnvironment) -> Option<TableCommand> {
        let (origin, entry_id) = env.redap_entry.clone()?;
        Some(TableCommand {
            origin,
            entry_id,
            kind: self,
        })
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
