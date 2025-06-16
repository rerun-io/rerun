use std::str::FromStr;

use re_types_core::{ArchetypeFieldName, ArchetypeName, ComponentDescriptor, ComponentType};

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
    UnexpectedComponentDescriptor(ComponentDescriptor),

    #[error("Found no component name")]
    MissingComponentDescriptor,

    #[error("Found trailing colon (:)")]
    TrailingColon,

    #[error("Found trailing hash (#)")]
    TrailingHash,

    #[error("Component descriptor doesn't have an archetype field name: {0:?}")]
    ComponentDescriptorMissesArchetypeFieldName(String),

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

        let mut component_descriptor = None;
        let mut instance = None;

        // Parse `:Points3D:Color#colors` suffix:
        if let Some(first_colon) = tokens.iter().position(|&token| token == ":") {
            let component_descriptor_tokens = &tokens[first_colon + 1..];
            if component_descriptor_tokens.is_empty() {
                return Err(PathParseError::TrailingColon);
            }

            let archetype_name_delimiter = &component_descriptor_tokens
                .iter()
                .position(|&token| token == ":");
            let archetype_name = if let Some(archetype_name_delimiter) = archetype_name_delimiter {
                if *archetype_name_delimiter == component_descriptor_tokens.len() - 1 {
                    return Err(PathParseError::TrailingColon);
                }

                if let Some(&archetype_name) =
                    component_descriptor_tokens.get(archetype_name_delimiter - 1)
                {
                    if archetype_name.contains('.') {
                        Some(archetype_name.to_owned())
                    } else {
                        Some(format!("rerun.archetypes.{archetype_name}"))
                    }
                } else {
                    return Err(PathParseError::TrailingColon);
                }
            } else {
                None
            };

            let component_type_delimiter = &component_descriptor_tokens
                .iter()
                .position(|&token| token == "#");
            let component_type = if let Some(component_type_delimiter) = component_type_delimiter {
                if let Some(component_type) =
                    component_descriptor_tokens.get(component_type_delimiter + 1)
                {
                    Some(component_type.to_owned())
                } else {
                    return Err(PathParseError::TrailingHash);
                }
            } else {
                None
            };

            let archetype_field_name_tokens_start = archetype_name_delimiter.map_or(0, |t| t + 1);
            let archetype_field_name_tokens_end =
                component_type_delimiter.unwrap_or(component_descriptor_tokens.len());
            if archetype_field_name_tokens_start == archetype_field_name_tokens_end {
                return Err(PathParseError::ComponentDescriptorMissesArchetypeFieldName(
                    join(component_descriptor_tokens),
                ));
            }

            let archetype_field_name = join(
                &component_descriptor_tokens
                    [archetype_field_name_tokens_start..archetype_field_name_tokens_end],
            );
            component_descriptor = Some(ComponentDescriptor {
                archetype_field_name: ArchetypeFieldName::from(archetype_field_name),
                archetype_name: archetype_name.map(ArchetypeName::from),
                component_type: component_type.map(ComponentType::from),
            });

            tokens.truncate(first_colon);
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
            component_descriptor,
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
            instance,
            component_descriptor,
        } = DataPath::from_str(input)?;

        if let Some(instance) = instance {
            return Err(PathParseError::UnexpectedInstance(instance));
        }
        if let Some(component_descriptor) = component_descriptor {
            return Err(PathParseError::UnexpectedComponentDescriptor(
                component_descriptor,
            ));
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
            component_descriptor,
        } = DataPath::from_str(s)?;

        if let Some(instance) = instance {
            return Err(PathParseError::UnexpectedInstance(instance));
        }

        let Some(component_descriptor) = component_descriptor else {
            return Err(PathParseError::MissingComponentDescriptor);
        };

        Ok(Self {
            entity_path,
            component_descriptor,
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

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use re_types_core::ComponentDescriptor;

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
            Err(PathParseError::UnexpectedComponentDescriptor { .. })
        ));
        assert!(matches!(
            parse(r#"entity[#123]"#),
            Err(PathParseError::UnexpectedInstance(Instance(123)))
        ));

        assert_eq!(parse("hallådär"), Ok(entity_path_vec!("hallådär")));
    }

    #[test]
    fn test_parse_component_path() {
        assert_eq!(
            ComponentPath::from_str("world/points:colors"),
            Ok(ComponentPath {
                entity_path: EntityPath::from("world/points"),
                component_descriptor: ComponentDescriptor::partial("colors"),
            })
        );
        assert_eq!(
            ComponentPath::from_str("world/points:colors"),
            Ok(ComponentPath {
                entity_path: EntityPath::from("world/points"),
                component_descriptor: ComponentDescriptor::partial("colors"),
            })
        );
        assert_eq!(
            ComponentPath::from_str("world/points:My.Custom.Archetype:colors"),
            Ok(ComponentPath {
                entity_path: EntityPath::from("world/points"),
                component_descriptor: ComponentDescriptor::partial("colors")
                    .with_archetype_name("My.Custom.Archetype".into()),
            })
        );
        assert_eq!(
            ComponentPath::from_str("world/points:Points3D:colors"),
            Ok(ComponentPath {
                entity_path: EntityPath::from("world/points"),
                component_descriptor: ComponentDescriptor::partial("colors")
                    .with_archetype_name("rerun.archetypes.Points3D".into()),
            })
        );
        assert_eq!(
            ComponentPath::from_str("world/points:My.Custom.Archetype:colors#colors"),
            Ok(ComponentPath {
                entity_path: EntityPath::from("world/points"),
                component_descriptor: ComponentDescriptor::partial("colors")
                    .with_archetype_name("My.Custom.Archetype".into())
                    .with_component_type("colors".into()),
            })
        );
        assert_eq!(
            ComponentPath::from_str("world/points:Points3D:colors#my.custom.colors"),
            Ok(ComponentPath {
                entity_path: EntityPath::from("world/points"),
                component_descriptor: ComponentDescriptor::partial("colors")
                    .with_archetype_name("rerun.archetypes.Points3D".into())
                    .with_component_type("my.custom.colors".into()),
            })
        );

        assert_eq!(
            ComponentPath::from_str("world/points:"),
            Err(PathParseError::TrailingColon)
        );
        assert_eq!(
            ComponentPath::from_str("world/points"),
            Err(PathParseError::MissingComponentDescriptor)
        );
        assert_eq!(
            ComponentPath::from_str("world/points[#42]:rerun.components.Color"),
            Err(PathParseError::UnexpectedInstance(Instance(42)))
        );
        assert_eq!(
            ComponentPath::from_str("world/points:Points3D:"),
            Err(PathParseError::TrailingColon)
        );
        assert_eq!(
            ComponentPath::from_str("world/points:Points3D:my.custom.color#"),
            Err(PathParseError::TrailingHash)
        );
        assert_eq!(
            ComponentPath::from_str("world/points:Points3D:#colors"),
            Err(PathParseError::ComponentDescriptorMissesArchetypeFieldName(
                "Points3D:#colors".to_owned()
            ))
        );
    }

    #[test]
    fn test_parse_data_path() {
        assert_eq!(
            DataPath::from_str("world/points[#42]:colors"),
            Ok(DataPath {
                entity_path: EntityPath::from("world/points"),
                instance: Some(Instance(42)),
                component_descriptor: Some(ComponentDescriptor::partial("colors")),
            })
        );
        assert_eq!(
            DataPath::from_str("world/points:colors"),
            Ok(DataPath {
                entity_path: EntityPath::from("world/points"),
                instance: None,
                component_descriptor: Some(ComponentDescriptor::partial("colors")),
            })
        );
        assert_eq!(
            DataPath::from_str("world/points:Points3D:colors#my.custom.color"),
            Ok(DataPath {
                entity_path: EntityPath::from("world/points"),
                instance: None,
                component_descriptor: Some(
                    ComponentDescriptor::partial("colors")
                        .with_archetype_name("rerun.archetypes.Points3D".into())
                        .with_component_type("my.custom.color".into())
                ),
            })
        );
        assert_eq!(
            DataPath::from_str("world/points[#42]"),
            Ok(DataPath {
                entity_path: EntityPath::from("world/points"),
                instance: Some(Instance(42)),
                component_descriptor: None,
            })
        );
        assert_eq!(
            DataPath::from_str("world/points"),
            Ok(DataPath {
                entity_path: EntityPath::from("world/points"),
                instance: None,
                component_descriptor: None,
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
}
