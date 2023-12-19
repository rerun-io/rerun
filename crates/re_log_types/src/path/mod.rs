//! Every logged entity in Rerun is logged to an [`EntityPath`].
//!
//! The path is made up out of several [`EntityPathPart`]s,
//! which are just non-empty strings.

mod component_path;
mod data_path;
mod entity_path;
mod entity_path_expr;
mod entity_path_filter;
mod entity_path_part;
mod natural_ordering;
mod parse_path;

pub use component_path::ComponentPath;
pub use data_path::DataPath;
pub use entity_path::{EntityPath, EntityPathHash};
pub use entity_path_expr::EntityPathExpr;
pub use entity_path_filter::EntityPathFilter;
pub use entity_path_part::EntityPathPart;
pub use parse_path::PathParseError;

// ----------------------------------------------------------------------------

/// Build a `Vec<EntityPathPart>`:
/// ```
/// # use re_log_types::*;
/// let parts: Vec<EntityPathPart> = entity_path_vec!("foo", "bar");
/// ```
#[macro_export]
macro_rules! entity_path_vec {
    () => {
        // A vector of no elements that nevertheless has the expected concrete type.
        ::std::vec::Vec::<$crate::EntityPathPart>::new()
    };
    ($($part: expr),* $(,)?) => {
        vec![ $($crate::EntityPathPart::from($part),)+ ]
    };
}

/// Build an [`EntityPath`] from parts that are _not_ escaped:
///
/// ```
/// # use re_log_types::*;
/// let path: EntityPath = entity_path!("world", "my image!");
/// assert_eq!(path, EntityPath::parse_strict(r"world/my\ image\!").unwrap());
/// ```
#[macro_export]
macro_rules! entity_path {
    ($($part: expr),* $(,)?) => {
        $crate::EntityPath::from($crate::entity_path_vec![ $($part,)* ])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_path_macros_empty() {
        // If the type weren't constrained, this would be an ambiguous type error.
        assert_eq!(entity_path_vec!(), vec![]);
        assert_eq!(entity_path!(), EntityPath::from(vec![]));
    }
}
