use crate::{Origin, TimeRange};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RecordingEndpoint {
    pub origin: Origin,
    pub recording_id: String,
    pub time_range: Option<TimeRange>,
}

impl std::fmt::Display for RecordingEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/recording/{}", self.origin, self.recording_id)?;
        if let Some(time_range) = &self.time_range {
            write!(f, "?time_range={time_range}")?;
        }
        Ok(())
    }
}

impl RecordingEndpoint {
    pub fn new(origin: Origin, recording_id: String, time_range: Option<TimeRange>) -> Self {
        Self {
            origin,
            recording_id,
            time_range,
        }
    }

    /// Returns a [`RecordingEndpoint`] without the optional query part.
    pub fn without_query(&self) -> std::borrow::Cow<'_, Self> {
        let mut cow = std::borrow::Cow::Borrowed(self);
        if self.time_range.is_some() {
            cow.to_mut().time_range = None;
        }
        cow
    }
}

impl std::str::FromStr for RecordingEndpoint {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match crate::RedapUri::from_str(s)? {
            crate::RedapUri::Recording(endpoint) => Ok(endpoint),
            crate::RedapUri::Catalog(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
            crate::RedapUri::Proxy(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
        }
    }
}
