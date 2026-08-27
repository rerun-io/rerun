use re_log_types::StoreId;
use re_types_core::SegmentId;

use crate::{Error, Fragment, Origin, RedapUri};

/// Which resource of a dataset a [`DatasetUri`] points at.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes,
)]
pub enum DatasetResource {
    /// The segments of the dataset itself.
    #[default]
    Segments,

    /// The assets of the dataset, which are segments of its hidden asset dataset.
    Assets,
}

impl std::fmt::Display for DatasetResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Segments => "segments",
            Self::Assets => "assets",
        })
    }
}

impl std::str::FromStr for DatasetResource {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "segments" => Ok(Self::Segments),
            "assets" => Ok(Self::Assets),
            _ => Err(()),
        }
    }
}

/// URI pointing at a dataset, optionally at one of its segments.
///
/// Currently, the following formats are supported:
/// `<origin>/dataset/$DATASET_ID`
/// `<origin>/dataset/$DATASET_ID?segment_id=$SEGMENT_ID&time_range=$TIME_RANGE`
/// `<origin>/dataset/$DATASET_ID/assets`
/// `<origin>/dataset/$DATASET_ID/assets?segment_id=$ASSET_ID`
///
/// Without `segment_id` the uri points at the dataset as a whole.
/// `time_range` is optional. In the future we will add richer queries.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes)]
pub struct DatasetUri {
    pub origin: Origin,
    pub dataset_id: re_tuid::Tuid,
    pub resource: DatasetResource,

    // Query parameters: these affect what data is returned.
    /// `None` points at the dataset itself.
    pub segment_id: Option<SegmentId>,

    // Fragment parameters: these affect what the viewer focuses on:
    pub fragment: Fragment,
}

impl std::fmt::Display for DatasetUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            origin,
            dataset_id,
            resource,
            segment_id,
            fragment,
        } = self;

        write!(f, "{origin}/dataset/{dataset_id}")?;

        if *resource != DatasetResource::default() {
            write!(f, "/{resource}")?;
        }

        // ?query:
        if let Some(segment_id) = segment_id {
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

impl DatasetUri {
    pub fn new(
        origin: Origin,
        dataset_id: re_tuid::Tuid,
        resource: DatasetResource,
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
            (Some(s), None) | (None, Some(s)) => Some(s),

            (None, None) => None,

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
            resource,
            segment_id,
            fragment,
        })
    }

    /// Returns [`Self`] without any (optional) `#fragment`.
    pub fn without_fragment(mut self) -> Self {
        let Self {
            origin: _,     // Mandatory
            dataset_id: _, // Mandatory
            resource: _,   // Mandatory
            segment_id: _, // Selects which data to load
            fragment,
        } = &mut self;

        *fragment = Default::default();

        self
    }

    /// The store this segment is loaded into, or `None` without a segment.
    pub fn store_id(&self) -> Option<StoreId> {
        let segment_id = self.segment_id.clone()?;
        let dataset_id = re_log_types::EntryId::from(self.dataset_id);

        #[expect(deprecated)]
        let application_id = match self.resource {
            // The segments of a dataset all show the same kind of data, so they share a blueprint.
            DatasetResource::Segments => re_log_types::ApplicationId::from_entry_id(dataset_id),

            // The assets of a dataset have nothing in common, so each one gets its own blueprint.
            DatasetResource::Assets => {
                re_log_types::ApplicationId::from_asset(dataset_id, segment_id.as_str())
            }
        };

        Some(StoreId::new(
            re_log_types::StoreKind::Recording,
            application_id,
            segment_id,
        ))
    }
}

impl std::str::FromStr for DatasetUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::Dataset(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(Error::UnexpectedUri(s.to_owned()))
        }
    }
}

// --------------------------------

// Serialize as string:
impl serde::Serialize for DatasetUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for DatasetUri {
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
fn test_uri(resource: DatasetResource, segment_id: &str) -> DatasetUri {
    DatasetUri {
        origin: "rerun://127.0.0.1:1234".parse().expect("valid origin"),
        dataset_id: "1830B33B45B963E7774455beb91701ae"
            .parse()
            .expect("valid dataset id"),
        resource,
        segment_id: Some(segment_id.into()),
        fragment: Fragment::default(),
    }
}

/// The segments of a dataset all show the same kind of data, so they share an application id and
/// with it a blueprint.
#[test]
fn segments_of_a_dataset_share_an_application_id() {
    let first = test_uri(DatasetResource::Segments, "first")
        .store_id()
        .expect("names a segment");
    let second = test_uri(DatasetResource::Segments, "second")
        .store_id()
        .expect("names a segment");

    assert_eq!(first.application_id(), second.application_id());
    assert_ne!(first, second);
}

/// Each asset of a dataset gets its own application id, so that it shares a blueprint neither with
/// the other assets nor with the dataset's segments.
#[test]
fn assets_of_a_dataset_do_not_share_an_application_id() {
    let robot = test_uri(DatasetResource::Assets, "robot_mesh")
        .store_id()
        .expect("names a segment");
    let gripper = test_uri(DatasetResource::Assets, "gripper_mesh")
        .store_id()
        .expect("names a segment");
    let segment = test_uri(DatasetResource::Segments, "robot_mesh")
        .store_id()
        .expect("names a segment");

    assert_ne!(robot.application_id(), gripper.application_id());
    assert_ne!(robot.application_id(), segment.application_id());
    assert_ne!(robot, segment);
}

/// A uri that names no segment points at the dataset itself, and so at no store.
#[test]
fn a_dataset_without_a_segment_has_no_store_id() {
    let mut uri = test_uri(DatasetResource::Segments, "segment");
    uri.segment_id = None;

    assert_eq!(uri.store_id(), None);
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
