use std::str::FromStr;

use re_types::{components::InstanceKey, ComponentName};

use crate::{ComponentPath, DataPath, EntityPath, EntityPathPart, Index};

/// When parsing a [`DataPath`], it is important that we can distinguish the
/// component and index from the actual entity path. This requires
/// us to forbid certain characters in an entity part name.
/// For instance, in `foo/bar.baz`, is `baz` a component name, or part of the entity path?
/// So, when parsing a full [`DataPath`]s we are quite strict with what we allow.
/// But when parsing [`EntityPath`]s we want to be a bit more forgiving, so we
/// can accept things like `foo/bar.baz` and transform it into `foo/"bar.baz"`.
/// This allows user to do things like `log(f"foo/{filename}", my_mesh)` without
/// Rerun throwing a fit.
const STRICT_ENTITY_PATH_PARSING: bool = false;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PathParseError {
    #[error("Expected path, found empty string")]
    EmptyString,

    #[error("No entity path found")]
    MissingPath,

    #[error("Path had leading slash")]
    LeadingSlash,

    #[error("Missing closing quote (\")")]
    UnterminatedString,

    #[error("Bad escape sequence: {details}")]
    BadEscape { details: &'static str },

    #[error("Double-slashes with no part between")]
    DoubleSlash,

    #[error("Invalid sequence: {0:?} (expected positive integer)")]
    InvalidSequence(String),

    #[error("Missing slash (/)")]
    MissingSlash,

    #[error("Extra trailing slash (/)")]
    TrailingSlash,

    #[error("Empty part")]
    EmptyPart,

    #[error("Invalid character: {character:?} in entity path identifier {part:?}. Only ASCII characters, numbers, underscore, and dash are allowed. To put wild text in an entity path, surround it with double-quotes.")]
    InvalidCharacterInPart { part: String, character: char },

    #[error("Invalid instance key: {0:?} (expected '[#1234]')")]
    BadInstanceKey(String),

    #[error("Found an unexpected instance key: [#{}]", 0.0)]
    UnexpectedInstanceKey(InstanceKey),

    #[error("Found an unexpected trailing component name: {0:?}")]
    UnexpectedComponentName(ComponentName),

    #[error("Found no component name")]
    MissingComponentName,

    #[error("Found trailing dot (.)")]
    TrailingDot,
}

type Result<T, E = PathParseError> = std::result::Result<T, E>;

impl std::str::FromStr for DataPath {
    type Err = PathParseError;

    /// For instance:
    ///
    /// * `world/points`
    /// * `world/points.Color`
    /// * `world/points[#42]`
    /// * `world/points[#42].rerun.components.Color`
    fn from_str(path: &str) -> Result<Self, Self::Err> {
        if path.is_empty() {
            return Err(PathParseError::EmptyString);
        }

        // Start by looking for a component

        let mut tokens = tokenize(path)?;

        let mut component_name = None;
        let mut instance_key = None;

        // Parse `.rerun.components.Color` suffix:
        if let Some(dot) = tokens.iter().position(|&token| token == ".") {
            let component_tokens = &tokens[dot + 1..];

            if component_tokens.is_empty() {
                return Err(PathParseError::TrailingDot);
            } else if component_tokens.len() == 1 {
                component_name = Some(ComponentName::from(format!(
                    "rerun.components.{}",
                    join(component_tokens)
                )));
            } else {
                component_name = Some(ComponentName::from(join(component_tokens)));
            }
            tokens.truncate(dot);
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

        let parts = entity_path_parts_from_tokens(&tokens)?;

        // Validate names:
        for part in &parts {
            if let EntityPathPart::Name(name) = part {
                validate_name(name)?;
            }
        }

        let entity_path = EntityPath::from(parts);

        Ok(Self {
            entity_path,
            instance_key,
            component_name,
        })
    }
}

impl FromStr for EntityPath {
    type Err = PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if STRICT_ENTITY_PATH_PARSING {
            let DataPath {
                entity_path,
                instance_key,
                component_name,
            } = DataPath::from_str(s)?;

            if let Some(instance_key) = instance_key {
                return Err(PathParseError::UnexpectedInstanceKey(instance_key));
            }
            if let Some(component_name) = component_name {
                return Err(PathParseError::UnexpectedComponentName(component_name));
            }
            Ok(entity_path)
        } else {
            let mut parts = parse_entity_path_forgiving(s)?;

            for part in &mut parts {
                if let EntityPathPart::Name(name) = part {
                    if validate_name(name).is_err() {
                        // Quote this, e.g. `foo/invalid.name` -> `foo/"invalid.name"`
                        *part = EntityPathPart::Index(Index::String(name.to_string()));
                    }
                }
            }

            let path = EntityPath::from(parts);

            if path.to_string() != s {
                re_log::warn_once!("Found an invalid entity path '{s}' that was interpreted as '{path}'. Only ASCII characters, numbers, underscore, and dash are allowed in identifiers. To put wild text in an entity path, surround it with double-quotes.");
            }

            Ok(path)
        }
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
/// the path `foo/"Hallå Där!"`.
fn parse_entity_path_forgiving(path: &str) -> Result<Vec<EntityPathPart>, PathParseError> {
    if path.is_empty() {
        return Err(PathParseError::EmptyString);
    }

    if path == "/" {
        return Ok(vec![]); // special-case root entity
    }

    if path.starts_with('/') {
        return Err(PathParseError::LeadingSlash);
    }

    let mut bytes = path.as_bytes();

    let mut parts = vec![];

    while let Some(c) = bytes.first() {
        if *c == b'"' {
            // Look for the terminating quote ignoring escaped quotes (\"):
            let mut i = 1;
            loop {
                if i == bytes.len() {
                    return Err(PathParseError::UnterminatedString);
                } else if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2; // consume escape and what was escaped
                } else if bytes[i] == b'"' {
                    break;
                } else {
                    i += 1;
                }
            }

            let unescaped = unescape_string(std::str::from_utf8(&bytes[1..i]).unwrap())
                .map_err(|details| PathParseError::BadEscape { details })?;

            parts.push(EntityPathPart::Index(Index::String(unescaped)));

            bytes = &bytes[i + 1..]; // skip the closing quote

            match bytes.first() {
                None => {
                    break;
                }
                Some(b'/') => {
                    bytes = &bytes[1..];
                }
                _ => {
                    return Err(PathParseError::MissingSlash);
                }
            }
        } else {
            let end = bytes.iter().position(|&b| b == b'/').unwrap_or(bytes.len());
            parts.push(parse_part(std::str::from_utf8(&bytes[0..end]).unwrap())?);
            if end == bytes.len() {
                break;
            } else {
                bytes = &bytes[end + 1..]; // skip the /
            }
        }
    }

    Ok(parts)
}

fn entity_path_parts_from_tokens(mut tokens: &[&str]) -> Result<Vec<EntityPathPart>> {
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
        } else if token.starts_with('"') {
            assert!(token.ends_with('"'));
            let unescaped = unescape_string(&token[1..token.len() - 1])
                .map_err(|details| PathParseError::BadEscape { details })?;
            parts.push(EntityPathPart::Index(Index::String(unescaped)));
        } else {
            parts.push(parse_part(token)?);
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

fn tokenize(path: &str) -> Result<Vec<&str>> {
    let mut bytes = path.as_bytes();

    fn is_special_character(c: u8) -> bool {
        matches!(c, b'[' | b']' | b'.' | b'/')
    }

    let mut tokens = vec![];

    while let Some(c) = bytes.first() {
        if *c == b'"' {
            // Look for the terminating quote ignoring escaped quotes (\"):
            let mut i = 1;
            loop {
                if i == bytes.len() {
                    return Err(PathParseError::UnterminatedString);
                } else if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2; // consume escape and what was escaped
                } else if bytes[i] == b'"' {
                    break;
                } else {
                    i += 1;
                }
            }

            let token = &bytes[..i + 1]; // Include the closing quote
            tokens.push(token);
            bytes = &bytes[i + 1..]; // skip the closing quote
        } else if is_special_character(*c) {
            tokens.push(&bytes[..1]);
            bytes = &bytes[1..];
        } else {
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'"' || is_special_character(bytes[i]) {
                    break;
                }
                i += 1;
            }
            assert!(0 < i);
            tokens.push(&bytes[..i]);
            bytes = &bytes[i..];
        }
    }

    // Safety: we split at proper character boundaries
    Ok(tokens
        .iter()
        .map(|token| std::str::from_utf8(token).unwrap())
        .collect())
}

fn parse_part(s: &str) -> Result<EntityPathPart> {
    use std::str::FromStr as _;

    if s.is_empty() {
        Err(PathParseError::DoubleSlash)
    } else if let Some(s) = s.strip_prefix('#') {
        if let Ok(sequence) = u64::from_str(s) {
            Ok(EntityPathPart::Index(Index::Sequence(sequence)))
        } else {
            Err(PathParseError::InvalidSequence(s.into()))
        }
    } else if let Ok(integer) = i128::from_str(s) {
        Ok(EntityPathPart::Index(Index::Integer(integer)))
    } else if let Ok(uuid) = uuid::Uuid::parse_str(s) {
        Ok(EntityPathPart::Index(Index::Uuid(uuid)))
    } else {
        Ok(EntityPathPart::Name(s.into()))
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(PathParseError::EmptyPart);
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
            return Err(PathParseError::InvalidCharacterInPart {
                part: name.to_owned(),
                character: c,
            });
        }
    }

    Ok(())
}

fn unescape_string(input: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(c) = chars.next() {
                output.push(match c {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\"' | '\\' => c,
                    _ => {
                        return Err("Unknown escape sequence (\\)");
                    }
                });
            } else {
                return Err("Trailing escape (\\)");
            }
        } else {
            output.push(c);
        }
    }
    Ok(output)
}

#[test]
fn test_unescape_string() {
    let input = r#"Hello \"World\" /  \\ \n\r\t"#;
    let unescaped = unescape_string(input).unwrap();
    assert_eq!(unescaped, "Hello \"World\" /  \\ \n\r\t");
}

#[test]
fn test_parse_entity_path() {
    use crate::entity_path_vec;

    fn parse(s: &str) -> Result<Vec<EntityPathPart>> {
        EntityPath::from_str(s).map(|path| path.to_vec())
    }

    assert_eq!(parse(""), Err(PathParseError::EmptyString));
    assert_eq!(parse("/"), Ok(entity_path_vec!()));
    assert_eq!(parse("foo"), Ok(entity_path_vec!("foo")));
    assert_eq!(parse("/foo"), Err(PathParseError::LeadingSlash));
    assert_eq!(parse("foo/bar"), Ok(entity_path_vec!("foo", "bar")));
    assert_eq!(parse("foo//bar"), Err(PathParseError::DoubleSlash));
    assert_eq!(
        parse(r#"foo/"bar"/#123/-1234/6d046bf4-e5d3-4599-9153-85dd97218cb3"#),
        Ok(entity_path_vec!(
            "foo",
            Index::String("bar".into()),
            Index::Sequence(123),
            Index::Integer(-1234),
            Index::Uuid(uuid::Uuid::parse_str("6d046bf4-e5d3-4599-9153-85dd97218cb3").unwrap())
        ))
    );
    assert_eq!(
        parse(r#"foo/"bar""baz""#),
        Err(PathParseError::MissingSlash)
    );

    if STRICT_ENTITY_PATH_PARSING {
        assert_eq!(parse("foo/bar/"), Err(PathParseError::TrailingSlash));
        assert!(matches!(
            parse(r#"entity.component"#),
            Err(PathParseError::UnexpectedComponentName { .. })
        ));
        assert!(matches!(
            parse(r#"entity[#123]"#),
            Err(PathParseError::UnexpectedInstanceKey(InstanceKey(123)))
        ));
    } else {
        assert_eq!(
            EntityPath::from_str("foo/bar/").unwrap().to_string(),
            "foo/bar"
        );
        assert_eq!(
            EntityPath::from_str("foo/bar.baz").unwrap().to_string(),
            r#"foo/"bar.baz""#
        );
    }
}

#[test]
fn test_parse_component_path() {
    assert_eq!(
        ComponentPath::from_str("world/points.rerun.components.Color"),
        Ok(ComponentPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            component_name: "rerun.components.Color".into(),
        })
    );
    assert_eq!(
        ComponentPath::from_str("world/points.Color"),
        Ok(ComponentPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            component_name: "rerun.components.Color".into(),
        })
    );
    assert_eq!(
        ComponentPath::from_str("world/points.my.custom.color"),
        Ok(ComponentPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            component_name: "my.custom.color".into(),
        })
    );
    assert_eq!(
        ComponentPath::from_str("world/points."),
        Err(PathParseError::TrailingDot)
    );
    assert_eq!(
        ComponentPath::from_str("world/points"),
        Err(PathParseError::MissingComponentName)
    );
    assert_eq!(
        ComponentPath::from_str("world/points[#42].rerun.components.Color"),
        Err(PathParseError::UnexpectedInstanceKey(InstanceKey(42)))
    );
}

#[test]
fn test_parse_data_path() {
    assert_eq!(
        DataPath::from_str("world/points[#42].rerun.components.Color"),
        Ok(DataPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            instance_key: Some(InstanceKey(42)),
            component_name: Some("rerun.components.Color".into()),
        })
    );
    assert_eq!(
        DataPath::from_str("world/points.rerun.components.Color"),
        Ok(DataPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            instance_key: None,
            component_name: Some("rerun.components.Color".into()),
        })
    );
    assert_eq!(
        DataPath::from_str("world/points[#42]"),
        Ok(DataPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            instance_key: Some(InstanceKey(42)),
            component_name: None,
        })
    );
    assert_eq!(
        DataPath::from_str("world/points"),
        Ok(DataPath {
            entity_path: EntityPath::from_str("world/points").unwrap(),
            instance_key: None,
            component_name: None,
        })
    );

    // Check that we catch invalid characters in identifiers/names:
    assert!(matches!(
        DataPath::from_str(r#"hello there"#),
        Err(PathParseError::InvalidCharacterInPart { .. })
    ));
    assert!(matches!(
        DataPath::from_str(r#"hallådär"#),
        Err(PathParseError::InvalidCharacterInPart { .. })
    ));
    assert!(DataPath::from_str(r#"hello_there"#).is_ok());
    assert!(DataPath::from_str(r#"hello-there"#).is_ok());
}
