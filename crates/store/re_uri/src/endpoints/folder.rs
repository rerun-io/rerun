use crate::{Error, Origin, RedapUri};

/// `scheme://hostname:port/folder/<path>`
///
/// `path` is a dataset-name prefix using the dataset hierarchy separator (`.`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FolderUri {
    pub origin: Origin,
    pub path: String,
}

impl std::fmt::Display for FolderUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { origin, path } = self;

        // Use `url::Url` to handle percent-encoding of the path segment,
        // so unusual characters in dataset names round-trip safely.
        // We could use `percent-encoding` directly, but then we have to hardcode the set of allowed characters ourselves.
        let mut tmp = url::Url::parse("http://x/").expect("static URL is valid");
        tmp.path_segments_mut()
            .expect("absolute URL has a path")
            .clear()
            .push("folder")
            .push(path);
        let encoded_path = tmp.path(); // e.g. "/folder/perception.detection"

        write!(f, "{origin}{encoded_path}")
    }
}

impl FolderUri {
    pub fn new(origin: Origin, path: impl Into<String>) -> Self {
        Self {
            origin,
            path: path.into(),
        }
    }
}

impl std::str::FromStr for FolderUri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::Folder(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(Error::UnexpectedUri(s.to_owned()))
        }
    }
}

// Serialize as string:
impl serde::Serialize for FolderUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for FolderUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}
