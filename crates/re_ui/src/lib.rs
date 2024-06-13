//! Rerun GUI theme and helpers, built around [`egui`](https://www.egui.rs/).

mod command;
mod command_palette;
mod design_tokens;
mod layout_job_builder;
mod syntax_highlighting;

mod context_ext;
pub mod drag_and_drop;
pub mod icons;
pub mod list_item;
pub mod modal;
pub mod toasts;
mod ui_ext;

pub use self::{
    command::{UICommand, UICommandSender},
    command_palette::CommandPalette,
    context_ext::ContextExt,
    design_tokens::DesignTokens,
    icons::Icon,
    layout_job_builder::LayoutJobBuilder,
    syntax_highlighting::SyntaxHighlighting,
    ui_ext::UiExt,
};

// ---------------------------------------------------------------------------

/// If true, we fill the entire window, except for the close/maximize/minimize buttons in the top-left.
/// See <https://github.com/emilk/egui/pull/2049>
pub const FULLSIZE_CONTENT: bool = cfg!(target_os = "macos");

/// If true, we hide the native window decoration
/// (the top bar with app title, close button etc),
/// and instead paint our own close/maximize/minimize buttons.
pub const CUSTOM_WINDOW_DECORATIONS: bool = false; // !FULLSIZE_CONTENT; // TODO(emilk): https://github.com/rerun-io/rerun/issues/1063

/// If true, we show the native window decorations/chrome with the
/// close/maximize/minimize buttons and app title.
pub const NATIVE_WINDOW_BAR: bool = !FULLSIZE_CONTENT && !CUSTOM_WINDOW_DECORATIONS;

// ----------------------------------------------------------------------------

pub struct TopBarStyle {
    /// Height of the top bar
    pub height: f32,

    /// Extra horizontal space in the top left corner to make room for
    /// close/minimize/maximize buttons (on Mac)
    pub indent: f32,
}

/// The style of a label.
///
/// This should be used for all UI widgets that support these styles.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelStyle {
    /// Regular style for a label.
    #[default]
    Normal,

    /// Label displaying the placeholder text for a yet unnamed item (e.g. an unnamed space view).
    Unnamed,
}

// ----------------------------------------------------------------------------

/// Return a reference to the global design tokens structure.
pub fn design_tokens() -> &'static DesignTokens {
    use once_cell::sync::OnceCell;
    static DESIGN_TOKENS: OnceCell<DesignTokens> = OnceCell::new();
    DESIGN_TOKENS.get_or_init(DesignTokens::load)
}

/// Apply the Rerun design tokens to the given egui context and install image loaders.
pub fn apply_style_and_install_loaders(egui_ctx: &egui::Context) {
    egui_extras::install_image_loaders(egui_ctx);

    egui_ctx.include_bytes(
        "bytes://logo_dark_mode",
        include_bytes!("../data/logo_dark_mode.png"),
    );
    egui_ctx.include_bytes(
        "bytes://logo_light_mode",
        include_bytes!("../data/logo_light_mode.png"),
    );

    design_tokens().apply(egui_ctx);
}

/// Is this Ui in a resizable panel?
///
/// Used as a heuristic to figure out if it is safe to truncate text.
///
/// In a resizable panel, it is safe to truncate text if it doesn't fit,
/// because the user can just make the panel wider to see the full text.
///
/// In other places, we should never truncate text, because then the user
/// cannot read it all. In those places (when this functions returns `false`)
/// you should either wrap the text or let it grow the Ui it is in.
fn is_in_resizable_panel(ui: &egui::Ui) -> bool {
    re_tracing::profile_function!();

    let mut is_in_side_panel = false;

    for frame in ui.stack().iter() {
        if let Some(kind) = frame.kind() {
            if kind.is_area() {
                return false; // Our popups (tooltips etc) aren't resizable
            }
            if matches!(kind, egui::UiKind::LeftPanel | egui::UiKind::RightPanel) {
                is_in_side_panel = true;
            }
        }
    }

    if is_in_side_panel {
        true // Our side-panels are resizable
    } else {
        false // Safe fallback
    }
}
