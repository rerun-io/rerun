use re_log_types::{EntityPath, StoreKind};
use re_types::{components::Color, Component};
use re_viewer_context::{DefaultColor, ResolvedAnnotationInfo, ViewerContext};

pub fn lookup_override<C: Component>(
    data_result: &re_viewer_context::DataResult,
    ctx: &ViewerContext<'_>,
) -> Option<C> {
    data_result
        .property_overrides
        .as_ref()
        .and_then(|p| p.component_overrides.get(&C::name()))
        .and_then(|(store_kind, path)| match store_kind {
            StoreKind::Blueprint => ctx
                .store_context
                .blueprint
                .store()
                .query_latest_component::<C>(path, ctx.blueprint_query),
            StoreKind::Recording => ctx
                .entity_db
                .store()
                .query_latest_component::<C>(path, &ctx.current_query()),
        })
        .map(|c| c.value)
}

pub fn initial_override_color(entity_path: &EntityPath) -> Color {
    let default_color = DefaultColor::EntityPath(entity_path);

    let annotation_info = ResolvedAnnotationInfo::default();

    let color = annotation_info.color(None, default_color);

    let [r, g, b, a] = color.to_array();

    Color::from_unmultiplied_rgba(r, g, b, a)
}
