use crate::Origin;

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
        match crate::RedapUri::from_str(s)? {
            crate::RedapUri::Proxy(endpoint) => Ok(endpoint),
            crate::RedapUri::Recording(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
            crate::RedapUri::Catalog(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
        }
    }
}
