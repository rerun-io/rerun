use re_log_types::{DataPath, TimeCell, TimelineName};

/// We use the `#fragment` of the URI to point to a specific entity.
///
/// ```
/// # use re_uri::Fragment;
/// # let tests = [
///  "focus=/entity/path",
///  "focus=/entity/path[#42]",
///  "focus=/entity/path[#42]&when=log_tick@32",
///  "focus=/entity/path&when=log_time@2022-01-01T00:00:03.123456789Z",
///  "when=log_time@2022-01-01T00:00:03.123456789Z",
/// # ];
/// # for test in tests {
/// #     assert!(test.parse::<Fragment>().unwrap() != Fragment::default());
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Fragment {
    pub focus: Option<DataPath>,

    /// Select this timeline and this time
    pub when: Option<(TimelineName, TimeCell)>,
}

impl std::fmt::Display for Fragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { focus, when } = self;

        let mut did_write = false;

        if let Some(focus) = focus {
            write!(f, "focus={focus}")?;
            did_write = true;
        }

        if let Some((timeline, time_cell)) = when {
            if did_write {
                write!(f, "&")?;
            }
            write!(f, "when={timeline}@{time_cell}")?;
        }

        Ok(())
    }
}

impl std::str::FromStr for Fragment {
    type Err = String;

    fn from_str(fragment: &str) -> Result<Self, Self::Err> {
        let mut focus = None;
        let mut when = None;

        for part in split_on_unescaped_ampersand(fragment) {
            if let Some((key, value)) = split_at_first_unescaped_equals(part) {
                match key {
                    "focus" => match value.parse() {
                        Ok(path) => {
                            if focus.is_some() {
                                re_log::warn_once!(
                                    "Multiple paths set in uri #fragment {fragment:?}. Ignoring all but last."
                                );
                            }
                            focus = Some(path);
                        }
                        Err(err) => {
                            return Err(format!("Bad data path {part:?}: {err}"));
                        }
                    },
                    "when" => {
                        if let Some((timeline, time)) = value.split_once('@') {
                            let timeline = TimelineName::from(timeline);
                            match time.parse::<TimeCell>() {
                                Ok(time_cell) => {
                                    if when.is_some() {
                                        re_log::warn_once!(
                                            "Multiple times set in uri #fragment {fragment:?}. Ignoring all but last."
                                        );
                                    }
                                    when = Some((timeline, time_cell));
                                }
                                Err(err) => {
                                    return Err(format!("Bad time value {time:?}: {err}"));
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(format!(
                            "Unknown key {key:?}. Expected either 'focus' or 'time'"
                        ));
                    }
                }
            } else {
                re_log::warn_once!("Contained a part {part:?} without any equal sign in it");
            }
        }

        Ok(Self { focus, when })
    }
}

impl Fragment {
    /// Parse fragment, excluding hash
    pub fn parse_forgiving(fragment: &str) -> Self {
        match fragment.parse() {
            Ok(fragment) => fragment,
            Err(err) => {
                re_log::warn_once!("Failed to parse #fragment {fragment:?}: {err}");
                Self::default()
            }
        }
    }
}

/// Split on all '&' that is not immediately proceeded by '\':
fn split_on_unescaped_ampersand(str: &str) -> Vec<&str> {
    if str.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut start = 0;
    let bytes = str.as_bytes();

    for i in 0..bytes.len() {
        if bytes[i] == b'&' && (i == 0 || bytes[i - 1] != b'\\') {
            result.push(&str[start..i]);
            start = i + 1;
        }
    }

    result.push(&str[start..]);

    result
}

#[test]
fn test_split_on_unescaped_ampersand() {
    assert_eq!(split_on_unescaped_ampersand(""), Vec::<&str>::default());
    assert_eq!(split_on_unescaped_ampersand("foo"), vec!["foo"]);
    assert_eq!(split_on_unescaped_ampersand("a&b&c"), vec!["a", "b", "c"]);
    assert_eq!(split_on_unescaped_ampersand(r"a\&b&c"), vec![r"a\&b", "c"]);
    assert_eq!(
        split_on_unescaped_ampersand(r"a&b\&c&d"),
        vec!["a", r"b\&c", "d"]
    );
    assert_eq!(split_on_unescaped_ampersand(r"a\&b\&c"), vec![r"a\&b\&c"]);
    assert_eq!(split_on_unescaped_ampersand("a&&b"), vec!["a", "", "b"]);
    assert_eq!(split_on_unescaped_ampersand(r"a\&&b"), vec![r"a\&", "b"]);
}

/// Split a string at the first '=' that is not immediately preceded by '\'.
/// Returns `None` if no unescaped equals sign is found.
fn split_at_first_unescaped_equals(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();

    for i in 0..bytes.len() {
        if bytes[i] == b'=' && (i == 0 || bytes[i - 1] != b'\\') {
            return Some((&s[0..i], &s[i + 1..]));
        }
    }

    None
}

#[test]
fn test_split_underscore() {
    let test_cases = [
        ("key=value", Some(("key", "value"))),
        ("no_equals", None),
        ("escaped\\=equals", None),
        (
            "key\\=with_escape=value",
            Some(("key\\=with_escape", "value")),
        ),
        ("=", Some(("", ""))),
    ];

    for (s, expected) in test_cases {
        assert_eq!(split_at_first_unescaped_equals(s), expected);
    }
}

#[test]
fn test_parse_fragment() {
    let test_cases = [
        ("", Fragment::default()),
        (
            "focus=/entity/path",
            Fragment {
                focus: Some("/entity/path".parse().unwrap()),
                when: None,
            },
        ),
        (
            "focus=/entity/path&when=log_time@2022-01-01T00:00:03.123456789Z",
            Fragment {
                focus: Some("/entity/path".parse().unwrap()),
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
            },
        ),
        (
            "when=log_time@2022-01-01T00:00:03.123456789Z",
            Fragment {
                focus: None,
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
            },
        ),
    ];

    for (string, fragment) in test_cases {
        assert_eq!(fragment.to_string(), string);
        assert_eq!(string.parse::<Fragment>().unwrap(), fragment);
    }
}
