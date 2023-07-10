//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

mod auto_layout;
mod space_info;
mod space_view;
mod space_view_entity_picker;
mod space_view_heuristics;
mod space_view_highlights;
mod view_category;
mod viewport;
mod viewport_blueprint;
mod viewport_blueprint_ui;

pub mod blueprint_components;

pub use space_info::SpaceInfoCollection;
pub use space_view::SpaceViewBlueprint;
pub use view_category::ViewCategory;
pub use viewport::{Viewport, ViewportState};
pub use viewport_blueprint::ViewportBlueprint;

pub mod external {
    pub use re_space_view;
}

// ---------------------------------------------------------------------------

// TODO(andreas): This should be part of re_data_ui::item_ui.
pub mod item_ui {
    use re_data_ui::item_ui;
    use re_viewer_context::{Item, ViewerContext};

    use crate::space_view::SpaceViewBlueprint;

    pub fn space_view_button(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view: &SpaceViewBlueprint,
    ) -> egui::Response {
        let item = Item::SpaceView(space_view.id);
        let is_selected = ctx.selection().contains(&item);

        let response = ctx
            .re_ui
            .selectable_label_with_icon(
                ui,
                space_view.class(ctx.space_view_class_registry).icon(),
                space_view.display_name.clone(),
                is_selected,
            )
            .on_hover_text("Space View");
        item_ui::cursor_interact_with_selectable(ctx, response, item)
    }
}
