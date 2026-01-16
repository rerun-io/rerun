use re_log_types::{DataPath, TimeCell, TimelineName};

use crate::TimeSelection;

/// We use the `#fragment` of the URI to point to a specific entity or time.
///
/// ```
/// # use re_uri::Fragment;
/// # let tests = [
///  "selection=/entity/path",
///  "selection=/entity/path[#42]",
///  "selection=/entity/path[#42]&when=log_tick@32",
///  "selection=/entity/path&when=log_time@2022-01-01T00:00:03.123456789Z",
///  "when=log_time@2022-01-01T00:00:03.123456789Z",
/// # ];
/// # for test in tests {
/// #     assert!(test.parse::<Fragment>().unwrap() != Fragment::default());
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Fragment {
    pub selection: Option<DataPath>,

    /// Select this timeline and this time
    pub when: Option<(TimelineName, TimeCell)>,

    pub time_selection: Option<TimeSelection>,
}

impl std::fmt::Display for Fragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            selection,
            when,
            time_selection,
        } = self;

        #[expect(clippy::useless_let_if_seq)]
        let mut did_write = false;

        if let Some(selection) = selection {
            write!(f, "selection={selection}")?;
            did_write = true;
        }

        if let Some((timeline, time_cell)) = when {
            if did_write {
                write!(f, "&")?;
            }
            write!(f, "when={timeline}@",)?;
            time_cell.format_url(f)?;
            did_write = true;
        }

        if let Some(time_selection) = time_selection {
            if did_write {
                write!(f, "&")?;
            }
            write!(f, "time_selection=",)?;
            time_selection.format_url(f)?;
        }

        Ok(())
    }
}

impl std::str::FromStr for Fragment {
    type Err = String;

    fn from_str(fragment: &str) -> Result<Self, Self::Err> {
        let mut selection = None;
        let mut when = None;
        let mut time_selection = None;

        for part in split_on_unescaped_ampersand(fragment) {
            // If there isn't an equals in this part we skip it as it doesn't contain any data.
            if let Some((key, value)) = split_at_first_unescaped_equals(part) {
                match key {
                    "selection" => match value.parse() {
                        Ok(path) => {
                            // If there were selection fragments before this we override them.
                            selection = Some(path);
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
                                    // If there were when fragments before this we ignore them.
                                    when = Some((timeline, time_cell));
                                }
                                Err(err) => {
                                    return Err(format!("Bad time value {time:?}: {err}"));
                                }
                            }
                        }
                    }
                    "time_selection" => match value.parse() {
                        Ok(selection) => time_selection = Some(selection),
                        Err(err) => {
                            return Err(format!("Bad time selection {part:?}: {err}"));
                        }
                    },
                    _ => {
                        return Err(format!(
                            "Unknown key {key:?}. Expected either 'selection' or 'time'"
                        ));
                    }
                }
            }
        }

        Ok(Self {
            selection,
            when,
            time_selection,
        })
    }
}

impl Fragment {
    /// Parse fragment, excluding hash.
    ///
    /// Returns `Fragment::default()` if parsing fails.
    pub fn parse_forgiving(fragment: &str) -> Self {
        fragment.parse().unwrap_or_default()
    }

    /// True if this fragment doesn't contain any information.
    pub fn is_empty(&self) -> bool {
        // Keep this as a destruction so there is a compile error if a new field isn't handled here.
        let Self {
            selection,
            when,
            time_selection,
        } = self;

        selection.is_none() && when.is_none() && time_selection.is_none()
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
            "selection=/entity/path",
            Fragment {
                selection: Some("/entity/path".parse().unwrap()),
                when: None,
                time_selection: None,
            },
        ),
        (
            "selection=/entity/path&when=log_time@2022-01-01T00:00:03.123456789Z",
            Fragment {
                selection: Some("/entity/path".parse().unwrap()),
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
                time_selection: None,
            },
        ),
        (
            "when=log_time@2022-01-01T00:00:03.123456789Z",
            Fragment {
                selection: None,
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
                time_selection: None,
            },
        ),
        (
            "when=log_time@2022-01-01T00:00:03.123456789Z&time_selection=log_time@2022-01-01T00:00:01.123456789Z..2022-01-01T00:00:10.123456789Z",
            Fragment {
                selection: None,
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
                time_selection: Some(TimeSelection {
                    timeline: re_log_types::Timeline::log_time(),
                    range: re_log_types::AbsoluteTimeRange::new(
                        "2022-01-01T00:00:01.123456789Z"
                            .parse::<TimeCell>()
                            .unwrap()
                            .value,
                        "2022-01-01T00:00:10.123456789Z"
                            .parse::<TimeCell>()
                            .unwrap()
                            .value,
                    ),
                }),
            },
        ),
    ];

    for (string, fragment) in test_cases {
        assert_eq!(fragment.to_string(), string);
        assert_eq!(string.parse::<Fragment>().unwrap(), fragment);
    }

    let fail_cases = ["focus=/entity/path", "selection=/entity/path&foo=test"];

    for string in fail_cases {
        assert!(string.parse::<Fragment>().is_err());
    }
}
