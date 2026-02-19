use crate::{AppOptions, CommandSender, DisplayMode};

/// Application context that is shared across all parts of the viewer.
///
/// This context, in difference to [`crate::ViewerContext`] can exist for
/// any arbitrary state of the viewer. And not only when there is an open
/// recording.
pub struct AppContext<'a> {
    /// Set during tests (e.g. snapshot tests).
    ///
    /// Used to hide non-deterministic UI elements such as the current time.
    pub is_test: bool,

    pub memory_limit: re_memory::MemoryLimit,

    /// Global options for the whole viewer.
    pub app_options: &'a AppOptions,

    /// Runtime info about components and archetypes.
    pub reflection: &'a re_types_core::reflection::Reflection,

    /// The [`egui::Context`].
    pub egui_ctx: &'a egui::Context,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: &'a re_renderer::RenderContext,

    /// Interface for sending commands back to the app
    pub command_sender: &'a CommandSender,

    /// Registry of authenticated redap connections
    pub connection_registry: &'a re_redap_client::ConnectionRegistryHandle,

    /// The current display mode of the viewer.
    pub display_mode: &'a DisplayMode,

    /// Are we logged in to rerun cloud?
    pub auth_context: Option<&'a AuthContext>,
}

pub struct AuthContext {
    pub email: String,
}

impl AppContext<'_> {
    pub fn logged_in(&self) -> bool {
        self.auth_context.is_some()
    }
}
