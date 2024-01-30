use re_log_types::StoreKind;
use re_types::Component;
use re_viewer_context::ViewerContext;

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
