//! Provide query-centric access to the `re_arrow_store`
//! TODO(jleibs) better crate documentation.

pub mod dataframe_util;
mod query;
mod visit;

pub use self::query::{
    get_component_with_instances, query_entity_with_primary, ComponentWithInstances, EntityView,
    Result,
};
// Used for doc-tests
pub use self::query::__populate_example_store;
pub use self::visit::{iter_column, visit_component, visit_components2, visit_components3};
