use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::Timeline;
use re_types_core::Component;

pub(crate) fn validate_component<C: Component>(blueprint: &EntityDb) -> bool {
    let engine = blueprint.storage_engine();
    if let Some(data_type) = engine.store().lookup_datatype(&C::name()) {
        if data_type != &C::arrow2_datatype() {
            // If the schemas don't match, we definitely have a problem
            re_log::debug!(
                "Unexpected datatype for component {:?}.\nFound: {:#?}\nExpected: {:#?}",
                C::name(),
                data_type,
                C::arrow2_datatype()
            );
            return false;
        } else {
            // Otherwise, our usage of serde-fields means we still might have a problem
            // this can go away once we stop using serde-fields.
            // Walk the blueprint and see if any cells fail to deserialize for this component type.
            let query = LatestAtQuery::latest(Timeline::default());
            for path in blueprint.entity_paths() {
                if let Some(array) = engine
                    .cache()
                    .latest_at(&query, path, [C::name()])
                    .component_batch_raw(&C::name())
                {
                    if let Err(err) = C::from_arrow_opt(&*array) {
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
