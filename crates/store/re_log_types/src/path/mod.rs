//! Every logged entity in Rerun is logged to an [`EntityPath`].
//!
//! The path is made up out of several [`EntityPathPart`]s,
//! which are just non-empty strings.

mod component_path;
mod data_path;
mod entity_path;
mod entity_path_filter;
mod entity_path_part;
pub mod natural_ordering;
mod parse_path;

pub use component_path::ComponentPath;
pub use data_path::DataPath;
pub use entity_path::{EntityPath, EntityPathHash};
pub use entity_path_filter::{
    EntityPathFilter, EntityPathFilterError, EntityPathRule, EntityPathSubs, FilterEvaluation,
    ResolvedEntityPathFilter, ResolvedEntityPathRule, RuleEffect,
};
pub use entity_path_part::EntityPathPart;
pub use parse_path::{PathParseError, tokenize_by};

// ----------------------------------------------------------------------------

/// Reexports for use by macros to avoid depending on the caller's namespacing.
#[doc(hidden)]
pub mod __private {
    pub use ::std::{string, vec};
}

/// Build a `Vec<EntityPathPart>`:
/// ```
/// # #![no_std] // test that the macro does not depend on the std *prelude*
/// # extern crate std;
/// # fn main() {
/// # use std::vec::Vec;
/// # use re_log_types::*;
/// let parts: Vec<EntityPathPart> = entity_path_vec!("foo", 42, "my image!");
/// # }
/// ```
#[macro_export]
macro_rules! entity_path_vec {
    () => {
        // A vector of no elements that nevertheless has the expected concrete type.
       $crate::path::__private::vec::Vec::<$crate::EntityPathPart>::new()
    };
    ($($part: expr),* $(,)?) => {
        $crate::path::__private::vec![ $($crate::EntityPathPart::from(
            $crate::path::__private::string::ToString::to_string(&$part)
        ),)+ ]
    };
}

/// Build an [`EntityPath`] from parts that are _not_ escaped:
///
/// ```
/// # use re_log_types::*;
/// let path: EntityPath = entity_path!("world", 42, "my image!");
/// assert_eq!(path, EntityPath::parse_strict(r"world/42/my\ image\!").unwrap());
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
