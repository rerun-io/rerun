use crate::{Origin, RedapUri};

/// `scheme://hostname:port/catalog`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CatalogUri {
    pub origin: Origin,
}

impl std::fmt::Display for CatalogUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/catalog", self.origin)
    }
}

impl CatalogUri {
    pub fn new(origin: Origin) -> Self {
        Self { origin }
    }
}

impl std::str::FromStr for CatalogUri {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::Catalog(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(crate::Error::UnexpectedUri(s.to_owned()))
        }
    }
}

// Serialize as string:
impl serde::Serialize for CatalogUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for CatalogUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}
