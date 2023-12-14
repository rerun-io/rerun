use re_string_interner::InternedString;

use crate::PathParseError;

/// The different parts that make up an [`EntityPath`][crate::EntityPath].
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
    /// We use an interned string for fast copies, fast hashing, and to save memory.
    /// Note that `re_string_interner` never frees memory, but even if a user
    /// allocates 100k different entity parts (which is crazy many lot), the memory usage
    /// will still only be in the low megabytes.
    InternedString,
);

impl EntityPathPart {
    /// The given string is expected to be unescaped, i.e. any `\` is treated as a normal character.
    #[inline]
    pub fn new(unescaped_string: impl Into<InternedString>) -> Self {
        Self(unescaped_string.into())
    }

    /// Unescape the string, forgiving any syntax error with a best-effort approach.
    pub fn parse_forgiving(input: &str) -> Self {
        Self::parse_forgiving_with_warning(input, None)
    }

    /// Unescape the string, forgiving any syntax error with a best-effort approach.
    ///
    /// Returns a warnings for potentially serious problems:
    /// * any unknown escape sequences
    /// * unescaped spaces
    pub fn parse_forgiving_with_warning(
        input: &str,
        mut warnings: Option<&mut Vec<String>>,
    ) -> Self {
        let mut output = String::with_capacity(input.len());
        let mut chars = input.chars();

        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(c) = chars.next() {
                    match c {
                        'n' => {
                            output.push('\n');
                        }
                        'r' => {
                            output.push('\r');
                        }
                        't' => {
                            output.push('\t');
                        }
                        'u' => {
                            match parse_unicode_escape(&mut chars) {
                                Ok(c) => {
                                    output.push(c);
                                }
                                Err(s) => {
                                    // Invalid unicode escape: treat it as a (escaped) backslash
                                    output.push('\\');
                                    output.push('u');
                                    output.push_str(&s);
                                }
                            };
                        }
                        c if c.is_ascii_punctuation() || c == ' ' => {
                            output.push(c);
                        }
                        _ => {
                            if let Some(warnings) = warnings.as_mut() {
                                // We want to warn on this, because it could be a serious mistake, like
                                // passing a windows file path (`C:\Users\image.jpg`) as an entity path
                                warnings.push(format!("Unknown escape sequence: '\\{c}'"));
                            }
                            output.push(c);
                        }
                    }
                } else {
                    // Trailing escape: treat it as a (escaped) backslash
                    output.push('\\');
                }
            } else {
                if c.is_whitespace() {
                    if let Some(warnings) = warnings.as_mut() {
                        // This could be a sign of forgetting to split a string containing multiple entity paths.
                        warnings.push("Unescaped whitespace".to_owned());
                    }
                }

                output.push(c);
            }
        }

        Self::new(output)
    }

    /// Unescape the string, returning errors on wrongly escaped input.
    pub fn parse_strict(input: &str) -> Result<Self, PathParseError> {
        let mut output = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(c) = chars.next() {
                    match c {
                        'n' => {
                            output.push('\n');
                        }
                        'r' => {
                            output.push('\r');
                        }
                        't' => {
                            output.push('\t');
                        }
                        'u' => match parse_unicode_escape(&mut chars) {
                            Ok(c) => {
                                output.push(c);
                            }
                            Err(s) => return Err(PathParseError::InvalidUnicodeEscape(s)),
                        },
                        c if c.is_ascii_punctuation() || c == ' ' => {
                            output.push(c);
                        }
                        c => return Err(PathParseError::UnknownEscapeSequence(c)),
                    };
                } else {
                    return Err(PathParseError::TrailingBackslash);
                }
            } else if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
                output.push(c);
            } else {
                return Err(PathParseError::MissingEscape(c));
            }
        }
        Ok(Self::from(output))
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
                        // Rust-style unicode escape, e.g. `\u{262E}`.
                        s.push_str(&format!("\\u{{{:04X}}}", c as u32));
                    }
                };
            }
        }
        s
    }
}

/// Parses e.g. `{262E}`.
///
/// Returns the consumed input characters on fail.
fn parse_unicode_escape(input: &mut impl Iterator<Item = char>) -> Result<char, String> {
    let mut all_chars = String::new();
    for c in input {
        all_chars.push(c);
        if c == '}' || all_chars.len() == 6 {
            break;
        }
    }

    let chars = all_chars.as_str();

    let Some(chars) = chars.strip_prefix('{') else {
        return Err(all_chars);
    };
    let Some(chars) = chars.strip_suffix('}') else {
        return Err(all_chars);
    };

    if chars.len() != 4 {
        return Err(all_chars);
    }

    let Ok(num) = u32::from_str_radix(chars, 16) else {
        return Err(all_chars);
    };

    char::from_u32(num).ok_or(all_chars)
}

impl std::fmt::Display for EntityPathPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.escaped_string().fmt(f)
    }
}

impl From<InternedString> for EntityPathPart {
    #[inline]
    fn from(part: InternedString) -> Self {
        Self(part)
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
        Self(part.into())
    }
}

impl std::cmp::Ord for EntityPathPart {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Use natural ordering of strings, so that "image2" comes before "image10".
        super::natural_ordering::compare(self.unescaped_str(), other.unescaped_str())
    }
}

impl std::cmp::PartialOrd for EntityPathPart {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[test]
fn test_parse_entity_path_part() {
    // strict:
    for (input, expected) in [
        (r"Hallå", "Hallå"),
        (r"Hall\u{00E5}\n\r\t", "Hallå\n\r\t"),
        (r"Hello\ world\!", "Hello world!"),
    ] {
        let part = EntityPathPart::parse_strict(input).unwrap();
        assert_eq!(part.unescaped_str(), expected);
    }

    assert_eq!(
        EntityPathPart::parse_strict(r"\u{262E}"),
        Ok(EntityPathPart::from("☮"))
    );
    assert_eq!(
        EntityPathPart::parse_strict(r"\u{apa}! :D")
            .unwrap_err()
            .to_string(),
        r"Expected e.g. '\u{262E}', found: '\u{apa}'"
    );

    assert_eq!(
        EntityPathPart::parse_strict(r"\u{0001}")
            .unwrap()
            .unescaped_str(),
        "\u{0001}"
    );
    assert_eq!(
        EntityPathPart::parse_forgiving("☮").escaped_string(),
        r"\u{262E}"
    );

    // forgiving:
    for (input, expected) in [
        (r"Hello\", "Hello\\"),
        (r"\u{apa}\u{262E}", r"\u{apa}☮"),
        (
            r#"Hello \"World\" /  \\ \n\r\t \u{00E5}"#,
            "Hello \"World\" /  \\ \n\r\t å",
        ),
    ] {
        let part = EntityPathPart::parse_forgiving(input);
        assert_eq!(part.unescaped_str(), expected);
    }

    // roundtripping:
    for str in [r"\u{0001}", r"Hello\ world\!\ \u{262E}"] {
        assert_eq!(
            EntityPathPart::parse_strict(str).unwrap().escaped_string(),
            str
        );
    }
}
