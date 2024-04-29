use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::Timeline;
use re_types_core::Component;

pub(crate) fn validate_component<C: Component>(blueprint: &EntityDb) -> bool {
    let query = LatestAtQuery::latest(Timeline::default());

    if let Some(data_type) = blueprint
        .query_caches()
        .lookup_datatype(blueprint.store(), &C::name())
    {
        if data_type != &C::arrow_datatype() {
            // If the schemas don't match, we definitely have a problem
            re_log::debug!(
                "Unexpected datatype for component {:?}.\nFound: {:#?}\nExpected: {:#?}",
                C::name(),
                data_type,
                C::arrow_datatype()
            );
            return false;
        } else {
            // Otherwise, our usage of serde-fields means we still might have a problem
            // this can go away once we stop using serde-fields.
            // Walk the blueprint and see if any cells fail to deserialize for this component type.
            for path in blueprint.entity_paths() {
                if let Some(res) = blueprint
                    .query_caches()
                    .latest_at(blueprint.store(), &query, path, [C::name()])
                    .get(C::name())
                    .and_then(|res| res.cell(blueprint.resolver(), C::name()))
                {
                    if let Err(err) = res.try_to_native_mono::<C>() {
                        re_log::debug!(
                            "Failed to deserialize component {:?}: {:?}",
                            C::name(),
                            err
                        );
                        return false;
                    }
                }
            }
        }
    }
    true
}
