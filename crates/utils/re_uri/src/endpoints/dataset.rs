use crate::{Origin, RedapUri, TimeRange};

//TODO(ab): add `DatasetTableEndpoint`, the URI pointing at the "table view" of the dataset (aka. its partition table).

/// URI pointing at the data underlying a dataset.
///
/// Currently, the following format is supported:
/// `<origin>/dataset/$DATASET_ID/data?partition_id=$PARTITION_ID&time_range=$TIME_RANGE`
///
/// `partition_id` is mandatory, and `time_range` is optional. In the future, it will be extended to
/// richer queries.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DatasetDataEndpoint {
    pub origin: Origin,
    pub dataset_id: re_tuid::Tuid,

    pub partition_id: String,
    pub time_range: Option<TimeRange>,
}

impl std::fmt::Display for DatasetDataEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/dataset/{}", self.origin, self.dataset_id)?;

        // query (for now, partition_id is the only supported one and is mandatory)
        {
            write!(f, "?partition_id={}", self.partition_id)?;
        }

        // time range
        if let Some(time_range) = &self.time_range {
            write!(f, "&time_range={time_range}")?;
        }

        Ok(())
    }
}

impl DatasetDataEndpoint {
    pub fn new(
        origin: Origin,
        dataset_id: re_tuid::Tuid,
        partition_id: String,
        time_range: Option<TimeRange>,
    ) -> Self {
        Self {
            origin,
            dataset_id,
            partition_id,
            time_range,
        }
    }

    /// Returns a [`DatasetDataEndpoint`] without the optional query part.
    pub fn without_query(&self) -> std::borrow::Cow<'_, Self> {
        let mut cow = std::borrow::Cow::Borrowed(self);

        if self.time_range.is_some() {
            cow.to_mut().time_range = None;
        }

        cow
    }
}

impl std::str::FromStr for DatasetDataEndpoint {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match RedapUri::from_str(s)? {
            RedapUri::DatasetData(endpoint) => Ok(endpoint),
            RedapUri::Recording(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
            RedapUri::Catalog(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
            RedapUri::Proxy(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
        }
    }
}
