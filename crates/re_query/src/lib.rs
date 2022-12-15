//! Provide query-centric access to the `re_arrow_store`
//! TODO(jleibs) better crate documentation.

mod query;
pub use self::query::{get_component_with_instances, query_entity_with_primary};
// Used for doc-tests
pub use self::query::__populate_example_store;

mod visit;
pub use self::visit::{iter_column, visit_component, visit_components2};

pub mod dataframe_util;
