use re_entity_db::EntityDb;
use re_types_core::Component;

pub(crate) fn validate_component<C: Component>(blueprint: &EntityDb) -> bool {
    let engine = blueprint.storage_engine();
    if let Some(data_type) = engine.store().lookup_datatype(&C::name())
        && data_type != C::arrow_datatype()
    {
        // If the schemas don't match, we definitely have a problem
        re_log::debug!(
            "Unexpected datatype for component {:?}.\nFound: {}\nExpected: {}",
            C::name(),
            data_type,
            C::arrow_datatype()
        );
        return false;
    }

    true
}
