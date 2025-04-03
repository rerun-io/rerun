use crate::{Origin, RedapUri};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProxyEndpoint {
    pub origin: Origin,
}

impl std::fmt::Display for ProxyEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/proxy", self.origin)
    }
}

impl ProxyEndpoint {
    pub fn new(origin: Origin) -> Self {
        Self { origin }
    }
}

impl std::str::FromStr for ProxyEndpoint {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match RedapUri::from_str(s)? {
            RedapUri::Proxy(endpoint) => Ok(endpoint),
            RedapUri::Catalog(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
            RedapUri::DatasetData(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
        }
    }
}

// Serialize as string:
impl serde::Serialize for ProxyEndpoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ProxyEndpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse::<Self>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}
