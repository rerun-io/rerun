//! Rerun Viewport Panel
//!
//! This crate provides the central panel that contains all space views.

mod auto_layout;
mod space_info;
mod space_view;
mod space_view_entity_picker;
mod space_view_heuristics;
mod space_view_highlights;
mod view_bar_chart;
mod view_category;
mod view_tensor;
mod view_time_series;
mod viewport;

pub mod blueprint_components;

pub use space_info::SpaceInfoCollection;
pub use space_view::{SpaceViewBlueprint, SpaceViewState};
pub use view_category::ViewCategory;
pub use viewport::{Viewport, ViewportState};

// ---------------------------------------------------------------------------

// TODO(andreas): This should be part of re_data_ui::item_ui.
pub mod item_ui {
    use re_data_ui::item_ui;
    use re_viewer_context::{Item, SpaceViewId, ViewerContext};

    use crate::{space_view::SpaceViewBlueprint, view_category::ViewCategory};

    pub fn space_view_button(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space_view: &SpaceViewBlueprint,
    ) -> egui::Response {
        space_view_button_to(
            ctx,
            ui,
            space_view.display_name.clone(),
            space_view.id,
            space_view.category,
        )
    }

    pub fn space_view_button_to(
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        space_view_category: ViewCategory,
    ) -> egui::Response {
        let item = Item::SpaceView(space_view_id);
        let is_selected = ctx.selection().contains(&item);

        let response = ctx
            .re_ui
            .selectable_label_with_icon(ui, space_view_category.icon(), text, is_selected)
            .on_hover_text("Space View");
        item_ui::cursor_interact_with_selectable(ctx.selection_state_mut(), response, item)
    }
}
