use re_log_types::EntryId;

use crate::{Error, Origin, RedapUri};

/// Which resource of a dataset an [`EntryUri`] points at.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    Default,
)]
pub enum DatasetResource {
    #[default]
    Segments,
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

/// URI for a remote entry.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct EntryUri {
    pub origin: Origin,
    pub entry_id: EntryId,
    pub resource: DatasetResource,
}

impl std::fmt::Display for EntryUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            origin,
            entry_id,
            resource,
        } = self;
        write!(f, "{origin}/entry/{entry_id}")?;

        if *resource != DatasetResource::default() {
            write!(f, "/{resource}")?;
        }

        Ok(())
    }
}

impl EntryUri {
    pub fn new(origin: Origin, entry_id: EntryId, resource: DatasetResource) -> Self {
        Self {
            origin,
            entry_id,
            resource,
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
