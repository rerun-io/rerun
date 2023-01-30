use crate::{EntityPathComponent, Index};

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

    #[error("Double-slashes with no component between")]
    DoubleSlash,

    #[error("Invalid sequence: {0:?} (expected positive integer)")]
    InvalidSequence(String),

    #[error("Missing slash (/)")]
    MissingSlash,
}

/// Parses an entity path, e.g. `foo/bar/#1234/5678/"string index"/a6a5e96c-fd52-4d21-a394-ffbb6e5def1d`
pub fn parse_entity_path(path: &str) -> Result<Vec<EntityPathComponent>, PathParseError> {
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

    let mut components = vec![];

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

            components.push(EntityPathComponent::Index(Index::String(unescaped)));

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
            components.push(parse_component(
                std::str::from_utf8(&bytes[0..end]).unwrap(),
            )?);
            if end == bytes.len() {
                break;
            } else {
                bytes = &bytes[end + 1..]; // skip the /
            }
        }
    }

    Ok(components)
}

fn parse_component(s: &str) -> Result<EntityPathComponent, PathParseError> {
    use std::str::FromStr as _;

    if s.is_empty() {
        Err(PathParseError::DoubleSlash)
    } else if let Some(s) = s.strip_prefix('#') {
        if let Ok(sequence) = u64::from_str(s) {
            Ok(EntityPathComponent::Index(Index::Sequence(sequence)))
        } else {
            Err(PathParseError::InvalidSequence(s.into()))
        }
    } else if let Ok(integer) = i128::from_str(s) {
        Ok(EntityPathComponent::Index(Index::Integer(integer)))
    } else if let Ok(uuid) = uuid::Uuid::parse_str(s) {
        Ok(EntityPathComponent::Index(Index::Uuid(uuid)))
    } else {
        Ok(EntityPathComponent::Name(s.into()))
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

    assert_eq!(parse_entity_path(""), Err(PathParseError::EmptyString));
    assert_eq!(parse_entity_path("/"), Ok(entity_path_vec!()));
    assert_eq!(parse_entity_path("foo"), Ok(entity_path_vec!("foo")));
    assert_eq!(parse_entity_path("/foo"), Err(PathParseError::LeadingSlash));
    assert_eq!(
        parse_entity_path("foo/bar"),
        Ok(entity_path_vec!("foo", "bar"))
    );
    assert_eq!(
        parse_entity_path("foo//bar"),
        Err(PathParseError::DoubleSlash)
    );
    assert_eq!(
        parse_entity_path(r#"foo/"bar"/#123/-1234/6d046bf4-e5d3-4599-9153-85dd97218cb3"#),
        Ok(entity_path_vec!(
            "foo",
            Index::String("bar".into()),
            Index::Sequence(123),
            Index::Integer(-1234),
            Index::Uuid(uuid::Uuid::parse_str("6d046bf4-e5d3-4599-9153-85dd97218cb3").unwrap())
        ))
    );
    assert_eq!(
        parse_entity_path(r#"foo/"bar""baz""#),
        Err(PathParseError::MissingSlash)
    );
}
