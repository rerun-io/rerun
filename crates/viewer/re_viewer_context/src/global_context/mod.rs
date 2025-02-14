//! Ultimately the goal is to have hierarchy of contexts from general to
//! increasingly specific. Each part of the viewer should only have access a
//! narrow context. Unfortunately, right now everything is still very
//! intertwined with [`ViewerContext`](crate::ViewerContext) so we can't pull
//! the [`GlobalContext`] out of this crate yet.

mod app_options;
mod command_sender;
mod component_ui_registry;
mod item;

pub use app_options::AppOptions;

use crate::ViewClassRegistry;
pub use command_sender::{
    command_channel, CommandReceiver, CommandSender, SystemCommand, SystemCommandSender,
};
pub use component_ui_registry::{ComponentUiRegistry, ComponentUiTypes};
pub(crate) use item::resolve_mono_instance_path_item;
pub use item::Item;

/// Application context that is shared across all parts of the viewer.
pub struct GlobalContext<'a> {
    /// Global options for the whole viewer.
    pub app_options: &'a AppOptions,

    /// Runtime info about components and archetypes.
    ///
    /// The component placeholder values for components are to be used when [`crate::ComponentFallbackProvider::try_provide_fallback`]
    /// is not able to provide a value.
    ///
    /// ⚠️ In almost all cases you should not use this directly, but instead use the currently best fitting
    /// [`crate::ComponentFallbackProvider`] and call [`crate::ComponentFallbackProvider::fallback_for`] instead.
    pub reflection: &'a re_types_core::reflection::Reflection,

    /// How to display components.
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// Registry of all known classes of views.
    pub view_class_registry: &'a ViewClassRegistry,

    /// The [`egui::Context`].
    pub egui_ctx: &'a egui::Context,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: &'a re_renderer::RenderContext,

    /// Interface for sending commands back to the app
    pub command_sender: &'a CommandSender,
}
