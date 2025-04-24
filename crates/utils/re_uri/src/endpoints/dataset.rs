use re_log_types::StoreId;

use crate::{Error, Fragment, Origin, RedapUri, TimeRange};

/// URI pointing at the data underlying a dataset.
///
/// Currently, the following format is supported:
/// `<origin>/dataset/$DATASET_ID/data?partition_id=$PARTITION_ID&time_range=$TIME_RANGE`
///
/// `partition_id` is currently mandatory, and `time_range` is optional.
/// In the future we will add richer queries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DatasetDataUri {
    pub origin: Origin,
    pub dataset_id: re_tuid::Tuid,

    // Query parameters: these affect what data is returned.
    /// Currently mandatory.
    pub partition_id: String,
    pub time_range: Option<TimeRange>,

    // Fragment parameters: these affect what the viewer focuses on:
    pub fragment: Fragment,
}

impl std::fmt::Display for DatasetDataUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            origin,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        } = self;

        write!(f, "{origin}/dataset/{dataset_id}")?;

        // ?query:
        {
            write!(f, "?partition_id={partition_id}")?;
        }
        if let Some(time_range) = time_range {
            write!(f, "&time_range={time_range}")?;
        }

        // #fragment:
        let fragment = fragment.to_string();
        if !fragment.is_empty() {
            write!(f, "#{fragment}")?;
        }

        Ok(())
    }
}

impl DatasetDataUri {
    pub fn new(origin: Origin, dataset_id: re_tuid::Tuid, url: &url::Url) -> Result<Self, Error> {
        let mut partition_id = None;
        let mut time_range = None;

        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "partition_id" => {
                    partition_id = Some(value.to_string());
                }
                "time_range" => {
                    time_range = Some(value.parse::<TimeRange>()?);
                }
                _ => {
                    re_log::warn_once!("Unknown query parameter: {key}={value}");
                }
            }
        }

        let Some(partition_id) = partition_id else {
            return Err(Error::MissingPartitionId);
        };

        let mut fragment = Fragment::default();
        if let Some(string) = url.fragment() {
            fragment = Fragment::parse_forgiving(string);
        }

        Ok(Self {
            origin,
            dataset_id,
            partition_id,
            time_range,
            fragment,
        })
    }

    /// Returns [`Self`] without any (optional) `?query` or `#fragment`.
    pub fn without_query_and_fragment(mut self) -> Self {
        let Self {
            origin: _,       // Mandatory
            dataset_id: _,   // Mandatory
            partition_id: _, // Mandatory
            time_range,
            fragment,
        } = &mut self;

        *time_range = None;
        *fragment = Default::default();

        self
    }

    pub fn recording_id(&self) -> StoreId {
        StoreId::from_string(
            re_log_types::StoreKind::Recording,
            self.partition_id.clone(),
        )
    }
}

impl std::str::FromStr for DatasetDataUri {
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
impl serde::Serialize for DatasetDataUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for DatasetDataUri {
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
