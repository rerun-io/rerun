use std::str::FromStr;

use re_types_core::{components::InstanceKey, ComponentName};

use crate::{ComponentPath, DataPath, EntityPath, EntityPathPart};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PathParseError {
    #[error("Expected path, found empty string")]
    EmptyString,

    #[error("No entity path found")]
    MissingPath,

    #[error("Path had leading slash")]
    LeadingSlash,

    #[error("Double-slashes with no part between")]
    DoubleSlash,

    #[error("Missing slash (/)")]
    MissingSlash,

    #[error("Extra trailing slash (/)")]
    TrailingSlash,

    #[error("Empty part")]
    EmptyPart,

    #[error("Invalid instance key: {0:?} (expected '[#1234]')")]
    BadInstanceKey(String),

    #[error("Found an unexpected instance key: [#{}]", 0.0)]
    UnexpectedInstanceKey(InstanceKey),

    #[error("Found an unexpected trailing component name: {0:?}")]
    UnexpectedComponentName(ComponentName),

    #[error("Found no component name")]
    MissingComponentName,

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

type Result<T, E = PathParseError> = std::result::Result<T, E>;

impl std::str::FromStr for DataPath {
    type Err = PathParseError;

    /// For instance:
    ///
    /// * `world/points`
    /// * `world/points:Color`
    /// * `world/points[#42]`
    /// * `world/points[#42]:rerun.components.Color`
    fn from_str(path: &str) -> Result<Self, Self::Err> {
        if path.is_empty() {
            return Err(PathParseError::EmptyString);
        }

        let mut tokens = tokenize_data_path(path);

        let mut component_name = None;
        let mut instance_key = None;

        // Parse `:rerun.components.Color` suffix:
        if let Some(colon) = tokens.iter().position(|&token| token == ":") {
            let component_tokens = &tokens[colon + 1..];

            if component_tokens.is_empty() {
                return Err(PathParseError::TrailingColon);
            } else {
                let mut name = join(component_tokens);
                if !name.contains('.') {
                    name = format!("rerun.components.{name}");
                }
                component_name = Some(ComponentName::from(name));
            }
            tokens.truncate(colon);
        }

        // Parse `[#1234]` suffix:
        if let Some(bracket) = tokens.iter().position(|&token| token == "[") {
            let instance_key_tokens = &tokens[bracket..];
            if instance_key_tokens.len() != 3 || instance_key_tokens.last() != Some(&"]") {
                return Err(PathParseError::BadInstanceKey(join(instance_key_tokens)));
            }
            let instance_key_token = instance_key_tokens[1];
            if let Some(nr) = instance_key_token.strip_prefix('#') {
                if let Ok(nr) = u64::from_str(nr) {
                    instance_key = Some(InstanceKey(nr));
                } else {
                    return Err(PathParseError::BadInstanceKey(
                        instance_key_token.to_owned(),
                    ));
                }
            } else {
                return Err(PathParseError::BadInstanceKey(
                    instance_key_token.to_owned(),
                ));
            }
            tokens.truncate(bracket);
        }

        // The remaining tokens should all be separated with `/`:

        let parts = entity_path_parts_from_tokens_strict(&tokens)?;

        let entity_path = EntityPath::from(parts);

        Ok(Self {
            entity_path,
            instance_key,
            component_name,
        })
    }
}

/// ## Entity path parsing
/// When parsing a [`DataPath`], it is important that we can distinguish the
/// component and index from the actual entity path. This requires
/// us to forbid certain characters in an entity part name.
/// For instance, in `foo/bar.baz`, is `baz` a component name, or part of the entity path?
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
            instance_key,
            component_name,
        } = DataPath::from_str(input)?;

        if let Some(instance_key) = instance_key {
            return Err(PathParseError::UnexpectedInstanceKey(instance_key));
        }
        if let Some(component_name) = component_name {
            return Err(PathParseError::UnexpectedComponentName(component_name));
        }

        let normalized = entity_path.to_string();
        if normalized != input {
            re_log::warn_once!("The entity path '{input}' was not in the normalized form. It will be interpreted as '{normalized}'. See https://www.rerun.io/docs/concepts/entity-path for more");
        }

        Ok(entity_path)
    }

    /// Parses an entity path, handling any malformed input with a logged warning.
    ///
    /// For a strict parses, use [`Self::parse_strict`] instead.
    pub fn parse_forgiving(input: &str) -> Self {
        let parts = parse_entity_path_forgiving(input);
        let entity_path = EntityPath::from(parts);

        let normalized = entity_path.to_string();
        if normalized != input {
            re_log::warn_once!("The entity path '{input}' was not in the normalized form. It will be interpreted as '{normalized}'. See https://www.rerun.io/docs/concepts/entity-path for more");
        }

        entity_path
    }
}

impl FromStr for ComponentPath {
    type Err = PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DataPath {
            entity_path,
            instance_key,
            component_name,
        } = DataPath::from_str(s)?;

        if let Some(instance_key) = instance_key {
            return Err(PathParseError::UnexpectedInstanceKey(instance_key));
        }

        let Some(component_name) = component_name else {
            return Err(PathParseError::MissingComponentName);
        };

        Ok(ComponentPath {
            entity_path,
            component_name,
        })
    }
}

/// A very forgiving parsing of the given entity path.
///
/// Things like `foo/Hallå Där!` will be accepted, and transformed into
/// the path `foo/Hallå\ Där\!`.
fn parse_entity_path_forgiving(path: &str) -> Vec<EntityPathPart> {
    tokenize_entity_path(path)
        .into_iter()
        .filter(|&part| part != "/") // ignore duplicate slashes
        .map(EntityPathPart::parse_forgiving)
        .collect()
}

fn entity_path_parts_from_tokens_strict(mut tokens: &[&str]) -> Result<Vec<EntityPathPart>> {
    if tokens.is_empty() {
        return Err(PathParseError::MissingPath);
    }

    if tokens == ["/"] {
        return Ok(vec![]); // special-case root entity
    }

    if tokens[0] == "/" {
        return Err(PathParseError::LeadingSlash);
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

/// "/foo/bar" -> ["/", "foo", "/", "bar"]
fn tokenize_entity_path(path: &str) -> Vec<&str> {
    tokenize_by(path, &[b'/'])
}

/// "/foo/bar[#42]:Color" -> ["/", "foo", "/", "bar", "[", "#42:", "]", ":", "Color"]
fn tokenize_data_path(path: &str) -> Vec<&str> {
    tokenize_by(path, &[b'/', b'[', b']', b':'])
}

fn tokenize_by<'s>(path: &'s str, special_chars: &[u8]) -> Vec<&'s str> {
    #![allow(clippy::unwrap_used)]

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

    // Safety: we split at proper character boundaries
    tokens
        .iter()
        .map(|token| std::str::from_utf8(token).unwrap())
        .collect()
}

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
    assert_eq!(normalize("/foo/bar/"), "foo/bar");
    assert_eq!(normalize("/foo///bar//"), "foo/bar");
    assert_eq!(normalize("foo/bar:baz"), r#"foo/bar\:baz"#);
    assert_eq!(normalize("foo/42"), "foo/42");
    assert_eq!(normalize("foo/#bar/baz"), r##"foo/\#bar/baz"##);
    assert_eq!(normalize("foo/Hallå Där!"), r#"foo/Hallå\ Där\!"#);
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
    assert_eq!(parse("/foo"), Err(PathParseError::LeadingSlash));
    assert_eq!(parse("foo/bar"), Ok(entity_path_vec!("foo", "bar")));
    assert_eq!(parse("foo//bar"), Err(PathParseError::DoubleSlash));

    assert_eq!(parse("foo/bar/"), Err(PathParseError::TrailingSlash));
    assert!(matches!(
        parse(r#"entity:component"#),
        Err(PathParseError::UnexpectedComponentName { .. })
    ));
    assert!(matches!(
        parse(r#"entity[#123]"#),
        Err(PathParseError::UnexpectedInstanceKey(InstanceKey(123)))
    ));

    assert_eq!(parse("hallådär"), Ok(entity_path_vec!("hallådär")));
}

#[test]
fn test_parse_component_path() {
    assert_eq!(
        ComponentPath::from_str("world/points:rerun.components.Color"),
        Ok(ComponentPath {
            entity_path: EntityPath::from("world/points"),
            component_name: "rerun.components.Color".into(),
        })
    );
    assert_eq!(
        ComponentPath::from_str("world/points:Color"),
        Ok(ComponentPath {
            entity_path: EntityPath::from("world/points"),
            component_name: "rerun.components.Color".into(),
        })
    );
    assert_eq!(
        ComponentPath::from_str("world/points:my.custom.color"),
        Ok(ComponentPath {
            entity_path: EntityPath::from("world/points"),
            component_name: "my.custom.color".into(),
        })
    );
    assert_eq!(
        ComponentPath::from_str("world/points:"),
        Err(PathParseError::TrailingColon)
    );
    assert_eq!(
        ComponentPath::from_str("world/points"),
        Err(PathParseError::MissingComponentName)
    );
    assert_eq!(
        ComponentPath::from_str("world/points[#42]:rerun.components.Color"),
        Err(PathParseError::UnexpectedInstanceKey(InstanceKey(42)))
    );
}

#[test]
fn test_parse_data_path() {
    assert_eq!(
        DataPath::from_str("world/points[#42]:rerun.components.Color"),
        Ok(DataPath {
            entity_path: EntityPath::from("world/points"),
            instance_key: Some(InstanceKey(42)),
            component_name: Some("rerun.components.Color".into()),
        })
    );
    assert_eq!(
        DataPath::from_str("world/points:rerun.components.Color"),
        Ok(DataPath {
            entity_path: EntityPath::from("world/points"),
            instance_key: None,
            component_name: Some("rerun.components.Color".into()),
        })
    );
    assert_eq!(
        DataPath::from_str("world/points[#42]"),
        Ok(DataPath {
            entity_path: EntityPath::from("world/points"),
            instance_key: Some(InstanceKey(42)),
            component_name: None,
        })
    );
    assert_eq!(
        DataPath::from_str("world/points"),
        Ok(DataPath {
            entity_path: EntityPath::from("world/points"),
            instance_key: None,
            component_name: None,
        })
    );

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
