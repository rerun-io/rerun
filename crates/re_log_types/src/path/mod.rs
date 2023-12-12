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
/// A non-empty string.
///
/// Note that the contents of the string is NOT escaped,
/// so escaping needs to be done when printing this
/// (done by the `Display` impl).
///
/// Because of this, `EntityPathPart` does NOT implement `AsRef<str>` etc.
///
/// In the file system analogy, this is the name of a folder.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EntityPathPart(
    // TODO(emilk): consider other string types; e.g. interned strings, `Arc<str>`, …
    String,
);

impl EntityPathPart {
    /// The given string is expected to be unescaped, i.e. any `\` is treated as a normal character.
    #[inline]
    pub fn new(unescaped_string: impl Into<String>) -> Self {
        Self(unescaped_string.into())
    }

    /// The unescaped string.
    ///
    /// Use [`Self::escaped_string`] or `to_string` to escape it.
    #[inline]
    pub fn unescaped_str(&self) -> &str {
        &self.0
    }

    #[inline]
    pub fn escaped_string(&self) -> String {
        let mut s = String::with_capacity(self.0.len());
        for c in self.0.chars() {
            // Note: we print all unicode character (e.g. `åäö`) as is.
            let print_as_is = c.is_alphanumeric() || matches!(c, '_' | '-' | '.');

            if print_as_is {
                s.push(c);
            } else {
                match c {
                    '\n' => {
                        s.push_str("\\n");
                    }
                    '\r' => {
                        s.push_str("\\r");
                    }
                    '\t' => {
                        s.push_str("\\t");
                    }
                    c if c.is_ascii_punctuation() || c == ' ' => {
                        s.push('\\');
                        s.push(c);
                    }
                    c => {
                        // Rust-style unicode escape, e.g. `\u{2009}`.
                        s.push_str(&format!("\\u{{{:x}}}", c as u32));
                    }
                };
            }
        }
        s
    }

    /// Unescape the string, forgiving any syntax error with a best-effort approach.
    pub fn parse_forgiving(input: &str) -> Self {
        let mut output = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(c) = chars.next() {
                    output.push(match c {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => c,
                    });
                } else {
                    // Trailing escape: treat it as a (escaped) backslash
                    output.push('\\');
                }
            } else {
                output.push(c);
            }
        }

        Self::new(output)
    }
}

impl std::fmt::Display for EntityPathPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.escaped_string().fmt(f)
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

impl std::cmp::Ord for EntityPathPart {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Use natural ordering of strings, so that "image2" comes before "image10".
        natural_ordering::compare(self.unescaped_str(), other.unescaped_str())
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

    #[test]
    fn test_unescape_string() {
        for (input, expected) in [
            (r"Hello\ world!", "Hello world!"),
            (r"Hello\", "Hello\\"),
            (
                r#"Hello \"World\" /  \\ \n\r\t"#,
                "Hello \"World\" /  \\ \n\r\t",
            ),
        ] {
            let part = EntityPathPart::parse_forgiving(input);
            assert_eq!(part.unescaped_str(), expected);
        }
    }
}
