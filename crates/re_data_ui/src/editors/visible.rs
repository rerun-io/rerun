use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb};
use re_log_types::{EntityPath, Instance};
use re_types::blueprint::components::Visible;
use re_viewer_context::{UiLayout, ViewerContext};

#[allow(clippy::too_many_arguments)]
pub fn edit_visible(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _ui_layout: UiLayout,
    query: &LatestAtQuery,
    db: &EntityDb,
    entity_path: &EntityPath,
    override_path: &EntityPath,
    component: Option<&LatestAtComponentResults>,
    instance: &Instance,
) {
    let visible = component
        // TODO(#5607): what should happen if the promise is still pending?
        .and_then(|c| c.instance::<Visible>(db.resolver(), instance.get() as _))
        .unwrap_or_else(|| default_visible(ctx, query, db, entity_path));
    let mut edit_visible = visible;

    ui.scope(|ui| {
        ui.visuals_mut().widgets.hovered.expansion = 0.0;
        ui.visuals_mut().widgets.active.expansion = 0.0;
        ui.add(re_ui::toggle_switch(15.0, &mut edit_visible.0));
    });

    if edit_visible != visible {
        ctx.save_blueprint_component(override_path, &edit_visible);
    }
}

#[inline]
pub fn default_visible(
    _ctx: &ViewerContext<'_>,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
) -> Visible {
    Visible(true)
}
