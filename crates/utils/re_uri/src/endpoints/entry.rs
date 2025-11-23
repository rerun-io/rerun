use re_log_types::EntryId;

use crate::{Error, Origin, RedapUri};

/// URI for a remote entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct EntryUri {
    pub origin: Origin,
    pub prefix: String,
    pub entry_id: EntryId,
}

impl std::fmt::Display for EntryUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}/entry/{}", self.origin, self.prefix, self.entry_id)
    }
}

impl EntryUri {
    pub fn new(origin: Origin, prefix: String, entry_id: EntryId) -> Self {
        Self {
            origin,
            prefix,
            entry_id,
        }
    }
}

impl std::str::FromStr for EntryUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::Entry(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(Error::UnexpectedUri(s.to_owned()))
        }
    }
}
