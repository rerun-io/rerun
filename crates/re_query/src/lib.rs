//! Provide query-centric access to the `re_arrow_store`
//! TODO(jleibs) better crate documentation.

mod query;
pub use self::query::get_component_with_instance_ids;
pub use self::query::query_entity_with_primary;
