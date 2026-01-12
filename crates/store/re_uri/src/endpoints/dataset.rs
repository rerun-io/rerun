use re_log_types::StoreId;

use crate::{Error, Fragment, Origin, RedapUri};

/// URI pointing at the data underlying a dataset.
///
/// Currently, the following format is supported:
/// `<origin>/dataset/$DATASET_ID/data?segment_id=$SEGMENT_ID&time_range=$TIME_RANGE`
///
/// `segment_id` is currently mandatory, and `time_range` is optional.
/// In the future we will add richer queries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DatasetSegmentUri {
    pub origin: Origin,
    pub dataset_id: re_tuid::Tuid,

    // Query parameters: these affect what data is returned.
    /// Currently mandatory.
    pub segment_id: String,

    // Fragment parameters: these affect what the viewer focuses on:
    pub fragment: Fragment,
}

impl std::fmt::Display for DatasetSegmentUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            origin,
            dataset_id,
            segment_id,
            fragment,
        } = self;

        write!(f, "{origin}/dataset/{dataset_id}")?;

        // ?query:
        {
            write!(f, "?segment_id={segment_id}")?;
        }

        // #fragment:
        let fragment = fragment.to_string();
        if !fragment.is_empty() {
            write!(f, "#{fragment}")?;
        }

        Ok(())
    }
}

impl DatasetSegmentUri {
    pub fn new(origin: Origin, dataset_id: re_tuid::Tuid, url: &url::Url) -> Result<Self, Error> {
        let mut segment_id = None;
        let mut legacy_partition_id = None;

        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                // Accept legacy `partition_id` query parameter.
                "partition_id" => {
                    legacy_partition_id = Some(value.to_string());
                }

                "segment_id" => {
                    segment_id = Some(value.to_string());
                }
                _ => {
                    // We ignore unknown query keys that may be from urls from prior/newer versions.
                }
            }
        }

        let segment_id = match (segment_id, legacy_partition_id) {
            (Some(s), None) | (None, Some(s)) => s,

            (None, None) => {
                return Err(Error::MissingSegmentId);
            }

            (Some(_), Some(_)) => {
                return Err(Error::AmbiguousSegmentId);
            }
        };

        let fragment = if let Some(string) = url.fragment() {
            Fragment::parse_forgiving(string)
        } else {
            Fragment::default()
        };

        Ok(Self {
            origin,
            dataset_id,
            segment_id,
            fragment,
        })
    }

    /// Returns [`Self`] without any (optional) `?query` or `#fragment`.
    pub fn without_query_and_fragment(mut self) -> Self {
        let Self {
            origin: _,     // Mandatory
            dataset_id: _, // Mandatory
            segment_id: _, // Mandatory
            fragment,
        } = &mut self;

        *fragment = Default::default();

        self
    }

    /// Returns [`Self`] without any (optional) `#fragment`.
    pub fn without_fragment(mut self) -> Self {
        let Self {
            origin: _,     // Mandatory
            dataset_id: _, // Mandatory
            segment_id: _, // Mandatory
            fragment,
        } = &mut self;

        *fragment = Default::default();

        self
    }

    pub fn store_id(&self) -> StoreId {
        StoreId::new(
            re_log_types::StoreKind::Recording,
            self.dataset_id.to_string(),
            self.segment_id.clone(),
        )
    }
}

impl std::str::FromStr for DatasetSegmentUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::DatasetData(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(Error::UnexpectedUri(s.to_owned()))
        }
    }
}

// --------------------------------

// Serialize as string:
impl serde::Serialize for DatasetSegmentUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for DatasetSegmentUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}

// --------------------------------

#[test]
fn test_url() {
    // Test how `+` is encoded.

    let url = url::Url::parse("http://www.example.com/foo?time=+42&foo=%2B1337").unwrap();

    assert_eq!(url.query(), Some("time=+42&foo=%2B1337"));

    let query_pairs = url
        .query_pairs()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect::<Vec<_>>();

    assert_eq!(
        query_pairs
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect::<Vec<_>>(),
        vec![("time", " 42"), ("foo", "+1337")]
    );
}
