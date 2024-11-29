//! UI utilities related to the viewport blueprint.
//!
//! Current this is mainly the add space view or container modal.

use parking_lot::Mutex;

use re_viewer_context::{ContainerId, ViewerContext};

use crate::ViewportBlueprint;
mod add_space_view_or_container_modal;

use add_space_view_or_container_modal::AddSpaceViewOrContainerModal;

static ADD_SPACE_VIEW_OR_CONTAINER_MODAL: once_cell::sync::Lazy<
    Mutex<AddSpaceViewOrContainerModal>,
> = once_cell::sync::Lazy::new(|| Mutex::new(AddSpaceViewOrContainerModal::default()));

pub fn add_space_view_or_container_modal_ui(
    ctx: &ViewerContext<'_>,
    viewport: &ViewportBlueprint,
    ui: &egui::Ui,
) {
    // give a chance to the modal to be drawn
    ADD_SPACE_VIEW_OR_CONTAINER_MODAL
        .lock()
        .ui(ui.ctx(), ctx, viewport);
}

pub fn show_add_space_view_or_container_modal(target_container: ContainerId) {
    ADD_SPACE_VIEW_OR_CONTAINER_MODAL
        .lock()
        .open(target_container);
}
