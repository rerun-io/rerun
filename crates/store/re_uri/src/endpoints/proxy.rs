use crate::{Origin, RedapUri};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProxyUri {
    pub origin: Origin,
}

impl std::fmt::Display for ProxyUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/proxy", self.origin)
    }
}

impl ProxyUri {
    pub fn new(origin: Origin) -> Self {
        Self { origin }
    }
}

impl std::str::FromStr for ProxyUri {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let RedapUri::Proxy(uri) = RedapUri::from_str(s)? {
            Ok(uri)
        } else {
            Err(crate::Error::UnexpectedUri(s.to_owned()))
        }
    }
}

// Serialize as string:
impl serde::Serialize for ProxyUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ProxyUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}
