//! Every logged entity in Rerun is logged to an [`EntityPath`].
//!
//! The path is made up out of several [`EntityPathPart`]s,
//! each of which is either a name ([`EntityPathPart::Name`])
//! or an [`Index`].
//!
//! The [`Index`]es are for tables, arrays etc.

mod component_path;
mod data_path;
mod entity_path;
mod entity_path_expr;
mod entity_path_impl;
mod natural_ordering;
mod parse_path;

pub use component_path::ComponentPath;
pub use data_path::DataPath;
pub use entity_path::{EntityPath, EntityPathHash};
pub use entity_path_expr::EntityPathExpr;
pub use entity_path_impl::EntityPathImpl;
pub use parse_path::PathParseError;

// ----------------------------------------------------------------------------

/// The different parts that make up an [`EntityPath`].
///
/// In the file system analogy, this is the name of a folder.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPathPart(
    // TODO(emilk): consider other string types; e.g. interned strings, `Arc<str>`, …
    String,
);

impl EntityPathPart {
    #[inline]
    pub fn new(string: impl Into<String>) -> Self {
        Self(string.into())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EntityPathPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut is_first_character = true;

        for c in self.0.chars() {
            let print_as_is = if is_first_character {
                // Escape punctutation if it is the first character,
                // so that we can use `-` for negation, and `.` to mean the current entity.
                c.is_alphanumeric()
            } else {
                c.is_alphanumeric() || matches!(c, '_' | '-' | '.')
            };

            if print_as_is {
                c.fmt(f)?;
            } else {
                match c {
                    '\n' => "\\n".fmt(f),
                    '\r' => "\\r".fmt(f),
                    '\t' => "\\t".fmt(f),
                    c if c.is_ascii_punctuation() || c == ' ' || c.is_alphanumeric() => {
                        write!(f, "\\{c}")
                    }
                    c => write!(f, "\\u{{{:x}}}", c as u32),
                }?;
            }

            is_first_character = false;
        }
        Ok(())
    }
}

impl From<&str> for EntityPathPart {
    #[inline]
    fn from(part: &str) -> Self {
        Self(part.into())
    }
}

impl From<String> for EntityPathPart {
    #[inline]
    fn from(part: String) -> Self {
        Self(part)
    }
}

impl AsRef<str> for EntityPathPart {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for EntityPathPart {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for EntityPathPart {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::cmp::Ord for EntityPathPart {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Use natural ordering of strings, so that "image2" comes before "image10".
        natural_ordering::compare(self.as_str(), other.as_str())
    }
}

impl std::cmp::PartialOrd for EntityPathPart {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

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

/// Build a `EntityPath`:
/// ```
/// # use re_log_types::*;
/// let path: EntityPath = entity_path!("foo", "bar");
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
