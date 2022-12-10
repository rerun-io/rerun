//! Provide query-centric access to the `re_arrow_store`
//! TODO(jleibs) better crate documentation.

mod query;

// Used for doc-tests
pub use self::query::__populate_example_store;
pub use self::query::{get_component_with_instances, query_entity_with_primary};

mod visit;
pub use self::visit::{iter_column, visit_component, visit_components2};

#[cfg(test)]
pub(crate) mod test_util;
