use egui::os::OperatingSystem;
use egui::{Key, KeyboardShortcut, Modifiers};
use smallvec::{SmallVec, smallvec};

use crate::context_ext::ContextExt as _;

/// Interface for sending [`UICommand`] messages.
pub trait UICommandSender {
    fn send_ui(&self, command: UICommand);
}

/// All the commands we support.
///
/// Most are available in the GUI,
/// some have keyboard shortcuts,
/// and all are visible in the [`crate::CommandPalette`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum UICommand {
    // Listed in the order they show up in the command palette by default!
    Open,
    OpenUrl,
    Import,

    CloseAllEntries,

    NextRecording,
    PreviousRecording,

    NavigateBack,
    NavigateForward,

    #[cfg(not(target_arch = "wasm32"))]
    Quit,

    OpenWebsite,
    OpenWebHelp,
    OpenRerunDiscord,

    ResetViewer,

    #[cfg(not(target_arch = "wasm32"))]
    OpenProfiler,

    #[cfg(not(target_arch = "wasm32"))]
    CaptureProfileTrace,

    TogglePanelStateOverrides,
    ToggleDevPanel,
    ToggleTopPanel,
    ToggleBlueprintPanel,
    ExpandBlueprintPanel,
    ToggleSelectionPanel,
    ExpandSelectionPanel,
    Settings,

    #[cfg(debug_assertions)]
    ToggleEguiDebugPanel,

    ToggleFullscreen,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomIn,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomOut,
    #[cfg(not(target_arch = "wasm32"))]
    ZoomReset,

    ToggleCommandPalette,

    // Dev-tools:
    #[cfg(not(target_arch = "wasm32"))]
    ScreenshotWholeApp,

    #[cfg(debug_assertions)]
    ResetEguiMemory,

    Share,
    CopyDirectLink,

    CopyTimeSelectionLink,

    CopyEntityHierarchy,

    // Graphics options:
    #[cfg(target_arch = "wasm32")]
    RestartWithWebGl,
    #[cfg(target_arch = "wasm32")]
    RestartWithWebGpu,

    // Redap commands
    AddRedapServer,
}

impl UICommand {
    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Open => (
                "Open file…",
                "Open any supported files (.rrd, images, meshes, …) in a new recording",
            ),
            Self::OpenUrl => (
                "Open from URL…",
                "Open or navigate to data from any supported URL",
            ),
            Self::Import => (
                "Import into current recording…",
                "Import any supported files (.rrd, images, meshes, …) in the current recording",
            ),

            Self::CloseAllEntries => (
                "Close all recordings",
                "Close all open current recording (unsaved data will be lost)",
            ),

            Self::NextRecording => ("Next recording", "Switch to the next open recording"),
            Self::PreviousRecording => (
                "Previous recording",
                "Switch to the previous open recording",
            ),

            Self::NavigateBack => ("Back in history", "Go back in history"),
            Self::NavigateForward => ("Forward in history", "Go forward in history"),

            #[cfg(not(target_arch = "wasm32"))]
            Self::Quit => ("Quit", "Close the Rerun Viewer"),

            Self::OpenWebsite => ("rerun.io", "Visit our homepage"),
            Self::OpenWebHelp => (
                "Docs",
                "Visit the docs on our website, with troubleshooting tips and more",
            ),
            Self::OpenRerunDiscord => (
                "Rerun Discord",
                "Visit the Rerun Discord server, where you can ask questions and get help",
            ),

            Self::ResetViewer => (
                "Reset Viewer",
                "Reset the Viewer to how it looked the first time you ran it, forgetting UI state and all stored blueprints, except the ones loaded from *.rbl resources",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::OpenProfiler => (
                "Open profiler",
                "Starts a profiler, showing what makes the viewer run slow",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::CaptureProfileTrace => (
                "Capture profile trace…",
                "Capture profiling data and save them as a .puffin file",
            ),

            Self::ToggleDevPanel => (
                "Toggle dev panel",
                "View developer stats like RAM usage inside Rerun Viewer",
            ),

            Self::TogglePanelStateOverrides => (
                "Toggle panel state overrides",
                "Toggle panel state between app blueprint and overrides",
            ),
            Self::ToggleTopPanel => ("Toggle top panel", "Toggle the top panel"),
            Self::ToggleBlueprintPanel => ("Toggle blueprint panel", "Toggle the left panel"),
            Self::ExpandBlueprintPanel => ("Expand blueprint panel", "Expand the left panel"),
            Self::ToggleSelectionPanel => ("Toggle selection panel", "Toggle the right panel"),
            Self::ExpandSelectionPanel => ("Expand selection panel", "Expand the right panel"),
            Self::Settings => ("Settings…", "Show the settings screen"),

            #[cfg(debug_assertions)]
            Self::ToggleEguiDebugPanel => (
                "Toggle egui debug panel",
                "View and change global egui style settings",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between windowed and fullscreen viewer",
            ),

            #[cfg(target_arch = "wasm32")]
            Self::ToggleFullscreen => (
                "Toggle fullscreen",
                "Toggle between full viewport dimensions and initial dimensions",
            ),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomIn => ("Zoom in", "Increases the UI zoom level"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomOut => ("Zoom out", "Decreases the UI zoom level"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomReset => (
                "Reset zoom",
                "Resets the UI zoom level to the operating system's default value",
            ),

            Self::ToggleCommandPalette => ("Command palette…", "Toggle the Command Palette"),

            #[cfg(not(target_arch = "wasm32"))]
            Self::ScreenshotWholeApp => (
                "Screenshot",
                "Copy screenshot of the whole app to clipboard",
            ),
            #[cfg(debug_assertions)]
            Self::ResetEguiMemory => (
                "Reset egui memory",
                "Reset egui memory, useful for debugging UI code.",
            ),

            Self::Share => ("Share…", "Share the current screen as a link"),
            Self::CopyDirectLink => (
                "Copy direct link",
                "Try to copy a shareable link to the current screen. This is not supported for all data sources & viewer states.",
            ),

            Self::CopyTimeSelectionLink => (
                "Copy link to selected time range",
                "Copy a link to the part of the active recording within the loop selection bounds.",
            ),

            Self::CopyEntityHierarchy => (
                "Copy entity hierarchy",
                "Copy the complete entity hierarchy tree of the currently active recording to the clipboard.",
            ),

            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGl => (
                "Restart with WebGL",
                "Reloads the webpage and force WebGL for rendering. All data will be lost.",
            ),
            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGpu => (
                "Restart with WebGPU",
                "Reloads the webpage and force WebGPU for rendering. All data will be lost.",
            ),

            Self::AddRedapServer => (
                "Connect to a server…",
                "Connect to a Redap server (experimental)",
            ),
        }
    }

    /// All keyboard shortcuts, with the primary first.
    // `os` is only used by OS-specific shortcuts (e.g. `Quit`), which are all native-only,
    // so it is unused on wasm:
    #[allow(clippy::allow_attributes, unused_variables)]
    pub fn kb_shortcuts(self, os: OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key)
        }

        fn cmd_alt(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, key)
        }

        match self {
            Self::Open => smallvec![cmd(Key::O)],
            // Some browsers have a "paste and go" action.
            // But unfortunately there's no standard shortcut for this.
            // Claude however thinks it's this one (it's not). Let's go with that anyways!
            Self::OpenUrl => smallvec![cmd_shift(Key::L)],
            Self::Import => smallvec![cmd_shift(Key::O)],
            Self::CloseAllEntries => smallvec![],

            Self::NextRecording => smallvec![cmd_alt(Key::ArrowDown)],
            Self::PreviousRecording => smallvec![cmd_alt(Key::ArrowUp)],

            Self::NavigateBack => smallvec![cmd(Key::OpenBracket)],
            Self::NavigateForward => smallvec![cmd(Key::CloseBracket)],

            #[cfg(not(target_arch = "wasm32"))]
            Self::Quit => {
                if os == OperatingSystem::Windows {
                    smallvec![KeyboardShortcut::new(Modifiers::ALT, Key::F4)]
                } else {
                    smallvec![cmd(Key::Q)]
                }
            }

            Self::OpenWebHelp => smallvec![],
            Self::OpenWebsite => smallvec![],
            Self::OpenRerunDiscord => smallvec![],

            Self::ResetViewer => smallvec![ctrl_shift(Key::R)],

            #[cfg(not(target_arch = "wasm32"))]
            Self::OpenProfiler => smallvec![ctrl_shift(Key::P)],
            #[cfg(not(target_arch = "wasm32"))]
            Self::CaptureProfileTrace => smallvec![],
            Self::ToggleDevPanel => smallvec![ctrl_shift(Key::M)],
            Self::TogglePanelStateOverrides => smallvec![],
            Self::ToggleTopPanel => smallvec![],
            Self::ToggleBlueprintPanel => smallvec![ctrl_shift(Key::B)],
            Self::ExpandBlueprintPanel => smallvec![],
            Self::ToggleSelectionPanel => smallvec![ctrl_shift(Key::S)],
            Self::ExpandSelectionPanel => smallvec![],
            Self::Settings => smallvec![cmd(Key::Comma)],

            #[cfg(debug_assertions)]
            Self::ToggleEguiDebugPanel => smallvec![ctrl_shift(Key::U)],

            Self::ToggleFullscreen => {
                if cfg!(target_arch = "wasm32") {
                    smallvec![]
                } else {
                    smallvec![key(Key::F11)]
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomIn => smallvec![egui::gui_zoom::kb_shortcuts::ZOOM_IN],
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomOut => smallvec![egui::gui_zoom::kb_shortcuts::ZOOM_OUT],
            #[cfg(not(target_arch = "wasm32"))]
            Self::ZoomReset => smallvec![egui::gui_zoom::kb_shortcuts::ZOOM_RESET],

            Self::ToggleCommandPalette => smallvec![cmd(Key::K), cmd(Key::P)],

            #[cfg(not(target_arch = "wasm32"))]
            Self::ScreenshotWholeApp => smallvec![],

            #[cfg(debug_assertions)]
            Self::ResetEguiMemory => smallvec![],

            Self::Share => smallvec![cmd(Key::L)],
            Self::CopyDirectLink => smallvec![],

            Self::CopyTimeSelectionLink => smallvec![],

            Self::CopyEntityHierarchy => smallvec![ctrl_shift(Key::E)],

            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGl => smallvec![],
            #[cfg(target_arch = "wasm32")]
            Self::RestartWithWebGpu => smallvec![],

            Self::AddRedapServer => smallvec![],
        }
    }

    /// Primary keyboard shortcut
    pub fn primary_kb_shortcut(self, os: OperatingSystem) -> Option<KeyboardShortcut> {
        self.kb_shortcuts(os).first().copied()
    }

    /// Return the keyboard shortcut for this command, nicely formatted
    // TODO(emilk): use Help/IconText instead
    pub fn formatted_kb_shortcut(self, egui_ctx: &egui::Context) -> Option<String> {
        // Note: we only show the primary shortcut to the user.
        // The fallbacks are there for people who have muscle memory for the other shortcuts.
        self.primary_kb_shortcut(egui_ctx.os())
            .map(|shortcut| egui_ctx.format_shortcut(&shortcut))
    }

    pub fn icon(self) -> Option<&'static crate::Icon> {
        match self {
            Self::OpenWebsite | Self::OpenWebHelp => Some(&crate::icons::EXTERNAL_LINK),
            Self::OpenRerunDiscord => Some(&crate::icons::DISCORD),
            _ => None,
        }
    }

    pub fn is_link(self) -> bool {
        matches!(self, Self::OpenWebHelp | Self::OpenRerunDiscord)
    }

    /// Listen for keyboard shortcuts of [`UICommand`]s only.
    ///
    /// The viewer should use [`super::listen_for_kb_shortcuts`] instead,
    /// which also matches recording commands.
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<Self> {
        use strum::IntoEnumIterator as _;

        let commands = Self::iter()
            .flat_map(|cmd| {
                cmd.kb_shortcuts(egui_ctx.os())
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, cmd))
            })
            .collect();

        super::consume_best_shortcut(egui_ctx, commands)
    }

    /// Show this command as a menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        command_sender: &impl UICommandSender,
    ) -> egui::Response {
        self.menu_button_ui_enabled(ui, true, command_sender)
    }

    /// Show this command as a (possibly disabled) menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui_enabled(
        self,
        ui: &mut egui::Ui,
        enabled: bool,
        command_sender: &impl UICommandSender,
    ) -> egui::Response {
        let button = self.menu_button(ui.ctx());
        let mut response = ui
            .add_enabled(enabled, button)
            .on_hover_text(self.tooltip());

        if self.is_link() {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            command_sender.send_ui(self);
            ui.close();
        }

        response
    }

    pub fn menu_button(self, egui_ctx: &egui::Context) -> egui::Button<'static> {
        let tokens = egui_ctx.tokens();

        let mut button = if let Some(icon) = self.icon() {
            egui::Button::image_and_text(
                icon.as_image()
                    .tint(tokens.label_button_icon_color)
                    .fit_to_exact_size(tokens.small_icon_size),
                self.text(),
            )
        } else {
            egui::Button::new(self.text())
        };

        if let Some(shortcut_text) = self.formatted_kb_shortcut(egui_ctx) {
            button = button.shortcut_text(shortcut_text);
        }

        button
    }

    /// Show name of command and how to activate it
    pub fn tooltip_ui(self, ui: &mut egui::Ui) {
        let os = ui.os();

        let (label, details) = self.text_and_tooltip();

        if let Some(shortcut) = self.primary_kb_shortcut(os) {
            crate::Help::new_without_title()
                .control(label, crate::IconText::from_keyboard_shortcut(os, shortcut))
                .ui(ui);
        } else {
            ui.label(label);
        }

        ui.set_max_width(220.0);
        ui.label(details);
    }
}
