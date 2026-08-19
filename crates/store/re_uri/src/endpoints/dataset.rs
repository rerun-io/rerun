use re_log_types::StoreId;
use re_types_core::SegmentId;

use crate::{DatasetResource, EntryUri, Error, Fragment, Origin, RedapUri};

/// Which of a dataset's segments a [`DatasetSegmentUri`] points at.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes,
)]
pub enum SegmentKind {
    /// The segments of the dataset itself.
    #[default]
    Segments,

    /// The assets of the dataset, which are segments of its hidden asset dataset.
    Assets,
}

impl std::fmt::Display for SegmentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Segments => "segments",
            Self::Assets => "assets",
        })
    }
}

impl std::str::FromStr for SegmentKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "segments" => Ok(Self::Segments),
            "assets" => Ok(Self::Assets),
            _ => Err(()),
        }
    }
}

/// URI pointing at the data underlying a dataset.
///
/// Currently, the following formats are supported:
/// `<origin>/dataset/$DATASET_ID?segment_id=$SEGMENT_ID&time_range=$TIME_RANGE`
/// `<origin>/dataset/$DATASET_ID/assets?segment_id=$ASSET_ID`
///
/// `segment_id` is currently mandatory, and `time_range` is optional.
/// In the future we will add richer queries.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes)]
pub struct DatasetSegmentUri {
    pub origin: Origin,
    pub dataset_id: re_tuid::Tuid,
    pub kind: SegmentKind,

    // Query parameters: these affect what data is returned.
    /// Currently mandatory.
    pub segment_id: SegmentId,

    // Fragment parameters: these affect what the viewer focuses on:
    pub fragment: Fragment,
}

impl std::fmt::Display for DatasetSegmentUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            origin,
            dataset_id,
            kind,
            segment_id,
            fragment,
        } = self;

        write!(f, "{origin}/dataset/{dataset_id}")?;

        if *kind != SegmentKind::default() {
            write!(f, "/{kind}")?;
        }

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
    pub fn new(
        origin: Origin,
        dataset_id: re_tuid::Tuid,
        kind: SegmentKind,
        url: &url::Url,
    ) -> Result<Self, Error> {
        let mut segment_id = None;
        let mut legacy_partition_id = None;

        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                // Accept legacy `partition_id` query parameter.
                "partition_id" => {
                    legacy_partition_id = Some(SegmentId::from(value));
                }

                "segment_id" => {
                    segment_id = Some(SegmentId::from(value));
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
            kind,
            segment_id,
            fragment,
        })
    }

    /// Returns [`Self`] without any (optional) `?query` or `#fragment`.
    pub fn without_query_and_fragment(mut self) -> Self {
        let Self {
            origin: _,     // Mandatory
            dataset_id: _, // Mandatory
            kind: _,       // Mandatory
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
            kind: _,       // Mandatory
            segment_id: _, // Mandatory
            fragment,
        } = &mut self;

        *fragment = Default::default();

        self
    }

    pub fn store_id(&self) -> StoreId {
        let dataset_id = re_log_types::EntryId::from(self.dataset_id);

        #[expect(deprecated)]
        let application_id = match self.kind {
            // The segments of a dataset all show the same kind of data, so they share a blueprint.
            SegmentKind::Segments => re_log_types::ApplicationId::from_entry_id(dataset_id),

            // The assets of a dataset have nothing in common, so each one gets its own blueprint.
            SegmentKind::Assets => {
                re_log_types::ApplicationId::from_asset(dataset_id, self.segment_id.as_str())
            }
        };

        StoreId::new(
            re_log_types::StoreKind::Recording,
            application_id,
            self.segment_id.clone(),
        )
    }

    pub fn dataset_url(&self) -> EntryUri {
        EntryUri {
            origin: self.origin.clone(),
            entry_id: re_log_types::EntryId::from(self.dataset_id),
            resource: match self.kind {
                SegmentKind::Segments => DatasetResource::Segments,
                SegmentKind::Assets => DatasetResource::Assets,
            },
        }
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

#[cfg(test)]
fn test_uri(kind: SegmentKind, segment_id: &str) -> DatasetSegmentUri {
    DatasetSegmentUri {
        origin: "rerun://127.0.0.1:1234".parse().expect("valid origin"),
        dataset_id: "1830B33B45B963E7774455beb91701ae"
            .parse()
            .expect("valid dataset id"),
        kind,
        segment_id: segment_id.into(),
        fragment: Fragment::default(),
    }
}

/// The segments of a dataset all show the same kind of data, so they share an application id and
/// with it a blueprint.
#[test]
fn segments_of_a_dataset_share_an_application_id() {
    let first = test_uri(SegmentKind::Segments, "first").store_id();
    let second = test_uri(SegmentKind::Segments, "second").store_id();

    assert_eq!(first.application_id(), second.application_id());
    assert_ne!(first, second);
}

/// Each asset of a dataset gets its own application id, so that it shares a blueprint neither with
/// the other assets nor with the dataset's segments.
#[test]
fn assets_of_a_dataset_do_not_share_an_application_id() {
    let robot = test_uri(SegmentKind::Assets, "robot_mesh").store_id();
    let gripper = test_uri(SegmentKind::Assets, "gripper_mesh").store_id();
    let segment = test_uri(SegmentKind::Segments, "robot_mesh").store_id();

    assert_ne!(robot.application_id(), gripper.application_id());
    assert_ne!(robot.application_id(), segment.application_id());
    assert_ne!(robot, segment);
}

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
