use std::str::FromStr;

use re_types_core::ComponentIdentifier;

use crate::{ComponentPath, DataPath, EntityPath, EntityPathPart, Instance};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PathParseError {
    #[error("Expected path, found empty string")]
    EmptyString,

    #[error("No entity path found")]
    MissingPath,

    #[error("Double-slashes with no part between")]
    DoubleSlash,

    #[error("Missing slash (/)")]
    MissingSlash,

    #[error("Extra trailing slash (/)")]
    TrailingSlash,

    #[error("Empty part")]
    EmptyPart,

    #[error("Invalid instance index: {0:?} (expected '[#1234]')")]
    BadInstance(String),

    #[error("Found an unexpected instance index: [#{}]", 0)]
    UnexpectedInstance(Instance),

    #[error("Found an unexpected trailing component descriptor: {0:?}")]
    UnexpectedComponent(ComponentIdentifier),

    #[error("Missing component")]
    MissingComponentIdentifier,

    #[error("Found trailing colon (:)")]
    TrailingColon,

    // Escaping:
    #[error("Unknown escape sequence: \\{0}")]
    UnknownEscapeSequence(char),

    #[error("String ended in backslash")]
    TrailingBackslash,

    #[error("{0:?} needs to be escaped as `\\{0}`")]
    MissingEscape(char),

    #[error("Expected e.g. '\\u{{262E}}', found: '\\u{0}'")]
    InvalidUnicodeEscape(String),
}

const _: () = assert!(
    std::mem::size_of::<PathParseError>() <= 48,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

type Result<T, E = PathParseError> = std::result::Result<T, E>;

impl std::str::FromStr for DataPath {
    type Err = PathParseError;

    /// For instance:
    ///
    /// * `/world/points`
    /// * `/world/points:Color`
    /// * `/world/points:Points3D:Color`
    /// * `/world/points:Points3D:Color#colors`
    /// * `/world/points[#42]`
    /// * `/world/points[#42]:rerun.components.Color`
    /// * `/world/points[#42]:Points3D:Color#colors`
    ///
    /// (the leadign slash is optional)
    fn from_str(path: &str) -> Result<Self, Self::Err> {
        if path.is_empty() {
            return Err(PathParseError::EmptyString);
        }

        let mut tokens = tokenize_data_path(path);

        let mut component = None;
        let mut instance = None;

        // Parse `:Points3D:Color` suffix:
        if let Some(first_colon) = tokens.iter().position(|&token| token == ":") {
            if let Some(component_tokens) = tokens.get(first_colon + 1..) {
                let component_tokens_end = component_tokens.len();
                if component_tokens
                    .len()
                    .checked_sub(1)
                    .is_none_or(|component_tokens_end| {
                        component_tokens.get(component_tokens_end) == Some(&":")
                    })
                {
                    return Err(PathParseError::TrailingColon);
                }

                let field = join(&component_tokens[0..component_tokens_end]);

                component = Some(field.into());

                tokens.truncate(first_colon);
            } else {
                return Err(PathParseError::TrailingColon);
            }
        }

        // Parse `[#1234]` suffix:
        if let Some(bracket) = tokens.iter().position(|&token| token == "[") {
            let instance_tokens = &tokens[bracket..];
            if instance_tokens.len() != 4 || instance_tokens.last() != Some(&"]") {
                return Err(PathParseError::BadInstance(join(instance_tokens)));
            }
            let nr = instance_tokens[2];
            if let Ok(nr) = u64::from_str(nr) {
                instance = Some(nr);
            } else {
                return Err(PathParseError::BadInstance(nr.to_owned()));
            }

            tokens.truncate(bracket);
        }

        // The remaining tokens should all be separated with `/`:

        let parts = entity_path_parts_from_tokens_strict(&tokens)?;

        let entity_path = EntityPath::from(parts);

        Ok(Self {
            entity_path,
            instance: instance.map(Into::into),
            component,
        })
    }
}

/// ## Entity path parsing
/// When parsing a [`DataPath`], it is important that we can distinguish the
/// component and index from the actual entity path. This requires
/// us to forbid certain characters in an entity part name.
/// For instance, in `foo/bar.baz`, is `baz` a component type, or part of the entity path?
/// So, when parsing a full [`DataPath`]s we are quite strict with what we allow.
/// But when parsing [`EntityPath`]s we want to be a bit more forgiving, so we
/// can accept things like `foo/bar.baz` and transform it into `foo/"bar.baz"`.
/// This allows user to do things like `log(f"foo/{filename}", my_mesh)` without
/// Rerun throwing a fit.
impl EntityPath {
    /// Parse an entity path from a string, with strict checks for correctness.
    ///
    /// Parses anything that `ent_path.to_string()` outputs.
    ///
    /// For a forgiving parse that accepts anything, use [`Self::parse_forgiving`].
    pub fn parse_strict(input: &str) -> Result<Self, PathParseError> {
        let DataPath {
            entity_path,
            instance,
            component,
        } = DataPath::from_str(input)?;

        if let Some(instance) = instance {
            return Err(PathParseError::UnexpectedInstance(instance));
        }
        if let Some(component) = component {
            return Err(PathParseError::UnexpectedComponent(component));
        }

        Ok(entity_path)
    }

    /// Parses an entity path, handling any malformed input with a logged warning.
    ///
    /// Things like `foo/Hallå Där!` will be accepted, and transformed into
    /// the path `foo/Hallå\ Där\!`.
    ///
    /// For a strict parses, use [`Self::parse_strict`] instead.
    pub fn parse_forgiving(input: &str) -> Self {
        let mut warnings = vec![];

        // TODO(#9193): Ideally we'd want to print a warning here, but that
        //              conflicts with how we construct entity paths in our non-Rust SDKs.
        // if input.starts_with(RESERVED_NAMESPACE_PREFIX)
        //     || input
        //         .strip_prefix("/")
        //         .is_some_and(|s| s.starts_with(RESERVED_NAMESPACE_PREFIX))
        // {
        //     re_log::warn_once!(
        //         "Entity path part starts with reserved namespace prefix `{RESERVED_NAMESPACE_PREFIX}`",
        //     );
        // }

        let parts: Vec<_> = tokenize_entity_path(input)
            .into_iter()
            .filter(|&part| part != "/") // ignore duplicate slashes
            .map(|part| EntityPathPart::parse_forgiving_with_warning(part, Some(&mut warnings)))
            .collect();

        let path = Self::from(parts);

        if let Some(warning) = warnings.first() {
            // We want to warn on some things, like
            // passing a windows file path (`C:\Users\image.jpg`) as an entity path,
            // which would result in a lot of unknown escapes.
            re_log::warn_once!(
                "When parsing the entity path {input:?}: {warning}. The path will be interpreted as {path}"
            );
        }

        path
    }
}

impl FromStr for ComponentPath {
    type Err = PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DataPath {
            entity_path,
            instance,
            component,
        } = DataPath::from_str(s)?;

        if let Some(instance) = instance {
            return Err(PathParseError::UnexpectedInstance(instance));
        }

        let Some(component) = component else {
            return Err(PathParseError::MissingComponentIdentifier);
        };

        Ok(Self {
            entity_path,
            component,
        })
    }
}

fn entity_path_parts_from_tokens_strict(mut tokens: &[&str]) -> Result<Vec<EntityPathPart>> {
    if tokens.is_empty() {
        return Err(PathParseError::MissingPath);
    }

    if tokens == ["/"] {
        return Ok(vec![]); // special-case root entity
    }

    if tokens[0] == "/" {
        // Leading slash is optional
        tokens = &tokens[1..];
    }

    let mut parts = vec![];

    loop {
        let token = tokens[0];
        tokens = &tokens[1..];

        if token == "/" {
            return Err(PathParseError::DoubleSlash);
        } else {
            parts.push(EntityPathPart::parse_strict(token)?);
        }

        if let Some(next_token) = tokens.first() {
            if *next_token == "/" {
                tokens = &tokens[1..];
                if tokens.is_empty() {
                    return Err(PathParseError::TrailingSlash);
                }
            } else {
                return Err(PathParseError::MissingSlash);
            }
        } else {
            break;
        }
    }

    Ok(parts)
}

fn join(tokens: &[&str]) -> String {
    let mut s = String::default();
    for token in tokens {
        s.push_str(token);
    }
    s
}

/// `"/foo/bar"` -> `["/", "foo", "/", "bar"]`
fn tokenize_entity_path(path: &str) -> Vec<&str> {
    tokenize_by(path, b"/")
}

/// `"/foo/bar[#42]:Points3D:Color#colors"` -> `["/", "foo", "/", "bar", "[", "#", "42:", "]", ":", "Points3D", ":", "Color", "#", "colors"]`
fn tokenize_data_path(path: &str) -> Vec<&str> {
    tokenize_by(path, b"/[]:#")
}

pub fn tokenize_by<'s>(path: &'s str, special_chars: &[u8]) -> Vec<&'s str> {
    #![expect(clippy::unwrap_used)]

    // We parse on bytes, and take care to only split on either side of a one-byte ASCII,
    // making the `from_utf8(…)`s below safe to unwrap.
    let mut bytes = path.as_bytes();

    let mut tokens = vec![];

    while !bytes.is_empty() {
        let mut i = 0;
        let mut is_in_escape = false;
        while i < bytes.len() {
            if !is_in_escape && special_chars.contains(&bytes[i]) {
                break;
            }
            is_in_escape = bytes[i] == b'\\';
            i += 1;
        }
        if i == 0 {
            // The first character was a special character, so we need to put it in its own token:
            i = 1;
        }
        tokens.push(&bytes[..i]);
        bytes = &bytes[i..];
    }

    // unwrap: we split at proper character boundaries
    tokens
        .iter()
        .map(|token| std::str::from_utf8(token).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fmt::{Debug, Display};
    use std::str::FromStr;

    use re_types_core::ComponentIdentifier;

    use super::Result;
    use crate::{ComponentPath, DataPath, EntityPath, EntityPathPart, Instance, PathParseError};

    #[test]
    fn test_parse_entity_path_forgiving() {
        use crate::entity_path_vec;

        fn parse(s: &str) -> Vec<EntityPathPart> {
            EntityPath::parse_forgiving(s).to_vec()
        }

        fn normalize(s: &str) -> String {
            EntityPath::parse_forgiving(s).to_string()
        }

        assert_eq!(parse(""), entity_path_vec!());
        assert_eq!(parse("/"), entity_path_vec!());
        assert_eq!(parse("foo"), entity_path_vec!("foo"));
        assert_eq!(parse("foo/bar"), entity_path_vec!("foo", "bar"));
        assert_eq!(
            parse(r#"foo/bar :/."#),
            entity_path_vec!("foo", "bar :", ".",)
        );
        assert_eq!(parse("hallådär"), entity_path_vec!("hallådär"));

        assert_eq!(normalize(""), "/");
        assert_eq!(normalize("/"), "/");
        assert_eq!(normalize("//"), "/");
        assert_eq!(normalize("/foo/bar/"), "/foo/bar");
        assert_eq!(normalize("/foo///bar//"), "/foo/bar");
        assert_eq!(normalize("foo/bar:baz"), r#"/foo/bar\:baz"#);
        assert_eq!(normalize("foo/42"), "/foo/42");
        assert_eq!(normalize("foo/#bar/baz"), r##"/foo/\#bar/baz"##);
        assert_eq!(normalize("foo/Hallå Där!"), r#"/foo/Hallå\ Där\!"#);
    }

    #[test]
    fn test_parse_entity_path_strict() {
        use crate::entity_path_vec;

        fn parse(s: &str) -> Result<Vec<EntityPathPart>> {
            EntityPath::parse_strict(s).map(|path| path.to_vec())
        }

        assert_eq!(parse(""), Err(PathParseError::EmptyString));
        assert_eq!(parse("/"), Ok(entity_path_vec!()));
        assert_eq!(parse("foo"), Ok(entity_path_vec!("foo")));
        assert_eq!(parse("/foo"), Ok(entity_path_vec!("foo")));
        assert_eq!(parse("foo/bar"), Ok(entity_path_vec!("foo", "bar")));
        assert_eq!(parse("/foo/bar"), Ok(entity_path_vec!("foo", "bar")));
        assert_eq!(parse("foo//bar"), Err(PathParseError::DoubleSlash));

        assert_eq!(parse("foo/bar/"), Err(PathParseError::TrailingSlash));
        assert!(matches!(
            parse(r#"entity:component"#),
            Err(PathParseError::UnexpectedComponent { .. })
        ));
        assert!(matches!(
            parse(r#"entity[#123]"#),
            Err(PathParseError::UnexpectedInstance(Instance(123)))
        ));

        assert_eq!(parse("hallådär"), Ok(entity_path_vec!("hallådär")));
    }

    #[track_caller]
    fn test_parse_ok<T: FromStr<Err = PathParseError> + Display + PartialEq + Debug>(
        src: &str,
        expected: &T,
    ) {
        let parsed = src.parse::<T>().expect("Parsing failed");

        assert_eq!(parsed, *expected, "Unexpected parsed result");

        let formatted = parsed.to_string();

        assert_eq!(
            src, formatted,
            "Source string is not the same as formatted string"
        );

        let re_parsed = formatted
            .parse::<T>()
            .expect("Parsing after formatting failed");

        assert_eq!(
            re_parsed, *expected,
            "Unexpected parsed result after parsing formatted"
        );
    }

    #[track_caller]
    fn test_parse_err<T: FromStr<Err = PathParseError> + Debug + PartialEq>(
        src: &str,
        expected: PathParseError,
    ) {
        assert_eq!(src.parse::<T>(), Err(expected));
    }

    #[test]
    fn test_parse_component_path() {
        #[track_caller]
        fn parse_ok(src: &str, entity_path: &str, component: &str) {
            test_parse_ok(
                src,
                &ComponentPath {
                    entity_path: EntityPath::from(entity_path),
                    component: ComponentIdentifier::from(component),
                },
            );
        }

        #[track_caller]
        fn parse_err(src: &str, expected: PathParseError) {
            test_parse_err::<ComponentPath>(src, expected);
        }

        parse_ok("/world/points:colors", "/world/points", "colors");

        parse_ok(
            "/world/points:My.Custom.Archetype.colors",
            "/world/points",
            "My.Custom.Archetype.colors",
        );

        parse_ok(
            "/world/points:Points3D:colors",
            "/world/points",
            "Points3D:colors",
        );
        parse_ok(
            "/world/points:My.Custom.Archetype.colors:colors",
            "/world/points",
            "My.Custom.Archetype.colors:colors",
        );

        parse_err("/world/points:", PathParseError::TrailingColon);
        parse_err("/world/points", PathParseError::MissingComponentIdentifier);
        parse_err(
            "/world/points[#42]:rerun.components.Color",
            PathParseError::UnexpectedInstance(Instance(42)),
        );
        parse_err("/world/points:Points3D:", PathParseError::TrailingColon);
    }

    #[test]
    fn test_parse_data_path() {
        #[track_caller]
        fn parse_ok(src: &str, entity_path: &str, instance: Option<u64>, component: Option<&str>) {
            test_parse_ok(
                src,
                &DataPath {
                    entity_path: EntityPath::from(entity_path),
                    instance: instance.map(Instance),
                    component: component.map(ComponentIdentifier::from),
                },
            );
        }
        parse_ok(
            "/world/points[#42]:colors",
            "/world/points",
            Some(42),
            Some("colors"),
        );
        parse_ok(
            "/world/points:colors",
            "/world/points",
            None,
            Some("colors"),
        );
        parse_ok(
            "/world/points:Points3D:colors",
            "/world/points",
            None,
            Some("Points3D:colors"),
        );
        parse_ok("/world/points[#42]", "/world/points", Some(42), None);
        parse_ok("/world/points", "/world/points", None, None);

        // Check that we catch invalid characters in identifiers/names:
        assert!(matches!(
            DataPath::from_str(r#"hello there"#),
            Err(PathParseError::MissingEscape(' '))
        ));
        assert!(DataPath::from_str(r#"hello_there"#).is_ok());
        assert!(DataPath::from_str(r#"hello-there"#).is_ok());
        assert!(DataPath::from_str(r#"hello.there"#).is_ok());
        assert!(DataPath::from_str(r#"hallådär"#).is_ok());
    }
}
