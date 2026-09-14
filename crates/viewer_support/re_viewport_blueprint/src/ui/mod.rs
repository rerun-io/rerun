//! UI utilities related to the viewport blueprint.
//!
//! Current this is mainly the add view or container modal.

use re_mutex::Mutex;
use re_viewer_context::{ContainerId, ViewerContext};

use crate::ViewportBlueprint;
mod add_view_or_container_modal;

use add_view_or_container_modal::AddViewOrContainerModal;

static ADD_VIEW_OR_CONTAINER_MODAL: std::sync::LazyLock<Mutex<AddViewOrContainerModal>> =
    std::sync::LazyLock::new(|| Mutex::new(AddViewOrContainerModal::default()));

pub fn add_view_or_container_modal_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &egui::Ui,
) {
    // give a chance to the modal to be drawn
    ADD_VIEW_OR_CONTAINER_MODAL
        .lock()
        .ui(ui.ctx(), ctx, viewport);
}

pub fn show_add_view_or_container_modal(target_container: ContainerId) {
    ADD_VIEW_OR_CONTAINER_MODAL.lock().open(target_container);
}
