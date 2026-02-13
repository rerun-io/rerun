use re_entity_db::EntityDb;
use re_types_core::Component;

pub(crate) fn validate_component<C: Component>(blueprint: &EntityDb) -> bool {
    let engine = blueprint.storage_engine();
    if let Some(actual_datatype) = engine
        .store()
        .has_mismatched_datatype_for_component_type(&C::name(), &C::arrow_datatype())
    {
        // If the schemas don't match, we definitely have a problem
        re_log::debug!(
            "Unexpected datatype for component {:?}.\nFound: {}\nExpected: {}",
            C::name(),
            actual_datatype,
            C::arrow_datatype()
        );
        return false;
    }

    true
}
