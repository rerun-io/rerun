use egui::KeyboardShortcut;
use egui::os::OperatingSystem;
use smallvec::{SmallVec, smallvec};

use super::CommandEnvironment;
use crate::context_ext::ContextExt as _;

/// Interface for sending [`RedapServerCommand`] messages.
pub trait RedapServerCommandSender {
    fn send_redap_server_command(&self, command: RedapServerCommand);
}

/// A command that acts on a specific Redap server.
///
/// Unlike [`super::UICommand`], these carry the [`re_uri::Origin`] of the server they act on,
/// so they can be used both from the command palette (acting on the selected server)
/// and from server context menus (acting on that specific server).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RedapServerCommand {
    /// The server this command acts on.
    pub origin: re_uri::Origin,

    /// What to do with the server.
    pub kind: RedapServerCommandKind,
}

/// What a [`RedapServerCommand`] does to its server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum RedapServerCommandKind {
    /// Refresh the contents (datasets & tables) of the server.
    Refresh,

    /// Open a modal to edit the URL and credentials of the server.
    Edit,

    /// Copy the URL of the server to the clipboard.
    CopyUrl,

    /// Remove the server from the redap browser.
    Remove,
}

impl RedapServerCommand {
    /// All commands that act on the given server.
    pub fn all_for_server(origin: &re_uri::Origin) -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator as _;
        let origin = origin.clone();
        RedapServerCommandKind::iter().map(move |kind| Self {
            origin: origin.clone(),
            kind,
        })
    }

    /// Does this command require the server to be editable
    /// (i.e. not the viewer's built-in catalog)?
    pub fn requires_editable_server(&self) -> bool {
        self.kind.requires_editable_server()
    }

    pub fn text(&self) -> &'static str {
        self.kind.text()
    }

    pub fn tooltip(&self) -> &'static str {
        self.kind.tooltip()
    }

    pub fn icon(&self) -> &'static crate::Icon {
        self.kind.icon()
    }

    /// Show this command as a menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        command_sender: &impl RedapServerCommandSender,
    ) -> egui::Response {
        let mut button = self
            .icon()
            .as_button_with_label(ui.ctx().tokens(), self.text());
        if let Some(shortcut_text) = self.kind.formatted_kb_shortcut(ui.ctx()) {
            button = button.shortcut_text(shortcut_text);
        }
        let response = ui.add(button).on_hover_text(self.tooltip());

        if response.clicked() {
            command_sender.send_redap_server_command(self);
            ui.close();
        }

        response
    }
}

impl RedapServerCommandKind {
    /// Does this command require the server to be editable
    /// (i.e. not the viewer's built-in catalog)?
    pub fn requires_editable_server(self) -> bool {
        match self {
            Self::Refresh | Self::CopyUrl => false,
            Self::Edit | Self::Remove => true,
        }
    }

    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Refresh => (
                "Refresh server",
                "Refresh the contents (datasets & tables) of the server",
            ),
            Self::Edit => ("Edit server…", "Edit the URL and credentials of the server"),
            Self::CopyUrl => ("Copy server URL", "Copy the URL of the server"),
            Self::Remove => ("Remove server", "Remove the server"),
        }
    }

    /// Pair this command with the selected server (from `env`) to make it dispatchable.
    ///
    /// Returns `None` when there is no selected server, or when the command requires an
    /// editable server but the selected one isn't (e.g. the viewer's built-in catalog).
    pub fn for_environment(self, env: &CommandEnvironment) -> Option<RedapServerCommand> {
        let origin = env.redap_server.clone()?;
        if self.requires_editable_server() && !env.has_editable_redap_server {
            return None;
        }
        Some(RedapServerCommand { origin, kind: self })
    }

    /// All keyboard shortcuts, with the primary first.
    ///
    /// Note: any command with a shortcut must be paired with the selected server when
    /// listening for shortcuts — see [`Self::for_environment`].
    pub fn kb_shortcuts(self, os: OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
        match self {
            // `Cmd-R` is the natural "reload" shortcut on Mac; elsewhere `F5` is (and `Cmd-R`
            // would clash with the browser's reload on web).
            Self::Refresh => super::refresh_shortcuts(os),
            Self::Edit | Self::CopyUrl | Self::Remove => smallvec![],
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

    pub fn icon(self) -> &'static crate::Icon {
        match self {
            Self::Refresh => &crate::icons::RESET,
            Self::Edit => &crate::icons::SETTINGS,
            Self::CopyUrl => &crate::icons::COPY,
            Self::Remove => &crate::icons::TRASH,
        }
    }
}
