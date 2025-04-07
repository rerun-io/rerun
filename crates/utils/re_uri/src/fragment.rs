use re_log_types::{DataPath, TimeCell, TimelineName};

/// We use the `#fragment` of the URI to point to a specific entity.
///
/// Format:
/// * `#/entity/path`
/// * `#/entity/path[#42]`
/// * `#/entity/path[#42]&log_tick=32`
/// * `#/entity/path&log_time=2022-01-01T00:00:03.123456789Z`
/// * `#log_time=2022-01-01T00:00:03.123456789Z`
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Fragment {
    pub data_path: Option<DataPath>,

    /// Select this timeline and this time
    pub when: Option<(TimelineName, TimeCell)>,
}

impl std::fmt::Display for Fragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { data_path, when } = self;

        let mut did_write = false;

        if let Some(data_path) = data_path {
            write!(f, "{data_path}")?;
            did_write = true;
        }

        if let Some((timeline, time_cell)) = when {
            if did_write {
                write!(f, "&")?;
            }
            write!(f, "{timeline}={time_cell}")?;
        }

        Ok(())
    }
}

impl Fragment {
    /// Parse fragment, excluding hash
    pub fn parse_forgiving(fragment: &str) -> Self {
        let mut data_path = None;
        let mut when = None;

        for part in split_on_unescaped_ampersand(fragment) {
            if let Some((timeline, time)) = part.split_once('=') {
                let timelinen = TimelineName::from(timeline);
                match time.parse::<TimeCell>() {
                    Ok(time_cell) => {
                        if when.is_some() {
                            re_log::warn_once!(
                                "Multiple times set in uri #fragment {fragment:?}. Ignoring all but last."
                            );
                        }
                        when = Some((timelinen, time_cell));
                    }
                    Err(err) => {
                        re_log::warn_once!(
                            "Bad time value {time:?} in uri #fragment {fragment:?}: {err}"
                        );
                        continue;
                    }
                }
            } else {
                match part.parse() {
                    Ok(path) => {
                        if data_path.is_some() {
                            re_log::warn_once!(
                                "Multiple paths set in uri #fragment {fragment:?}. Ignoring all but last."
                            );
                        }
                        data_path = Some(path);
                    }
                    Err(err) => {
                        re_log::warn_once!(
                            "Bad data path {part:?} in uri #fragment {fragment:?}: {err}"
                        );
                    }
                }
            }
        }

        Self { data_path, when }
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

#[test]
fn test_parse_fragment() {
    let test_cases = [
        ("", Fragment::default()),
        (
            "/entity/path",
            Fragment {
                data_path: Some("/entity/path".parse().unwrap()),
                when: None,
            },
        ),
        (
            "/entity/path&log_time=2022-01-01T00:00:03.123456789Z",
            Fragment {
                data_path: Some("/entity/path".parse().unwrap()),
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
            },
        ),
        (
            "log_time=2022-01-01T00:00:03.123456789Z",
            Fragment {
                data_path: None,
                when: Some((
                    "log_time".into(),
                    "2022-01-01T00:00:03.123456789Z".parse().unwrap(),
                )),
            },
        ),
    ];

    for (string, fragment) in test_cases {
        assert_eq!(fragment.to_string(), string);
        assert_eq!(Fragment::parse_forgiving(string), fragment);
    }
}
