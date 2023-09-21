use crate::{EntityPath, EntityPathPart, Index};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PathParseError {
    #[error("Expected path, found empty string")]
    EmptyString,

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

    #[error("Invalid character: {character:?} in entity path identifier {part:?}. Only ASCII characters, numbers, underscore, and dash are allowed. To put wild text in an entity path, surround it with double-quotes.")]
    InvalidCharacterInPart { part: String, character: char },
}

impl std::str::FromStr for EntityPath {
    type Err = crate::PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_entity_path_components(s).map(Self::new)
    }
}

/// Parses an entity path, e.g. `foo/bar/#1234/5678/"string index"/a6a5e96c-fd52-4d21-a394-ffbb6e5def1d`
fn parse_entity_path_components(path: &str) -> Result<Vec<EntityPathPart>, PathParseError> {
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
            let part_str = std::str::from_utf8(&bytes[0..end]).unwrap();
            parts.push(parse_part(part_str)?);
            if end == bytes.len() {
                break;
            } else {
                bytes = &bytes[end + 1..]; // skip the /
            }
        }
    }

    Ok(parts)
}

fn parse_part(s: &str) -> Result<EntityPathPart, PathParseError> {
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
        for c in s.chars() {
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
                return Err(PathParseError::InvalidCharacterInPart {
                    part: s.into(),
                    character: c,
                });
            }
        }
        Ok(EntityPathPart::Name(s.into()))
    }
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
fn test_parse_path() {
    use crate::entity_path_vec;

    assert_eq!(
        parse_entity_path_components(""),
        Err(PathParseError::EmptyString)
    );
    assert_eq!(parse_entity_path_components("/"), Ok(entity_path_vec!()));
    assert_eq!(
        parse_entity_path_components("foo"),
        Ok(entity_path_vec!("foo"))
    );
    assert_eq!(
        parse_entity_path_components("/foo"),
        Err(PathParseError::LeadingSlash)
    );
    assert_eq!(
        parse_entity_path_components("foo/bar"),
        Ok(entity_path_vec!("foo", "bar"))
    );
    assert_eq!(
        parse_entity_path_components("foo//bar"),
        Err(PathParseError::DoubleSlash)
    );
    assert_eq!(
        parse_entity_path_components(
            r#"foo/"bar"/#123/-1234/6d046bf4-e5d3-4599-9153-85dd97218cb3"#
        ),
        Ok(entity_path_vec!(
            "foo",
            Index::String("bar".into()),
            Index::Sequence(123),
            Index::Integer(-1234),
            Index::Uuid(uuid::Uuid::parse_str("6d046bf4-e5d3-4599-9153-85dd97218cb3").unwrap())
        ))
    );
    assert_eq!(
        parse_entity_path_components(r#"foo/"bar""baz""#),
        Err(PathParseError::MissingSlash)
    );

    // Check that we catch invalid characters in identifiers/names:
    assert!(matches!(
        parse_entity_path_components(r#"hello there"#),
        Err(PathParseError::InvalidCharacterInPart { .. })
    ));
    assert!(matches!(
        parse_entity_path_components(r#"hello.there"#),
        Err(PathParseError::InvalidCharacterInPart { .. })
    ));
    assert!(parse_entity_path_components(r#"hello_there"#).is_ok());
    assert!(parse_entity_path_components(r#"hello-there"#).is_ok());
}
