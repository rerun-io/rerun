use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb};
use re_log_types::{EntityPath, Instance};
use re_types::blueprint::components::Corner2D;
use re_viewer_context::{UiLayout, ViewerContext};

#[allow(clippy::too_many_arguments)]
pub fn edit_corner2d(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: &LatestAtComponentResults,
    instance: &Instance,
) {
    let corner = component
        // TODO(#5607): what should happen if the promise is still pending?
        .try_instance::<Corner2D>(db.resolver(), instance.get() as _)
        .unwrap_or_else(|| default_corner2d(ctx, query, db, entity_path));
    let mut edit_corner = corner;

    egui::ComboBox::from_id_source("corner2d")
        .selected_text(format!("{corner}"))
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(64.0);

            ui.selectable_value(
                &mut edit_corner,
                egui_plot::Corner::LeftTop.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::LeftTop)),
            );
            ui.selectable_value(
                &mut edit_corner,
                egui_plot::Corner::RightTop.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::RightTop)),
            );
            ui.selectable_value(
                &mut edit_corner,
                egui_plot::Corner::LeftBottom.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::LeftBottom)),
            );
            ui.selectable_value(
                &mut edit_corner,
                egui_plot::Corner::RightBottom.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::RightBottom)),
            );
        });

    if corner != edit_corner {
        ctx.save_blueprint_component(override_path, &edit_corner);
    }
}

#[inline]
pub fn default_corner2d(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> Corner2D {
    // TODO(#4194): Want to distinguish the space view this happens in.
    // TimeSeriesView: RightBottom
    // BarChart: RightTop
    // Need to make handling of editors a bit more powerful for this.
    // Rough idea right now is to have "default providers" which can be either a View or a Visualizer.
    // They then get queried with a ComponentName and return LatestAtComponentResults (or similar).
    Corner2D::RightBottom
}
