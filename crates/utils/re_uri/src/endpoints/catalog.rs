use crate::Origin;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CatalogEndpoint {
    pub origin: Origin,
}

impl std::fmt::Display for CatalogEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/catalog", self.origin)
    }
}

impl CatalogEndpoint {
    pub fn new(origin: Origin) -> Self {
        Self { origin }
    }
}

impl std::str::FromStr for CatalogEndpoint {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match crate::RedapUri::from_str(s)? {
            crate::RedapUri::Catalog(endpoint) => Ok(endpoint),
            crate::RedapUri::Recording(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
            crate::RedapUri::Proxy(endpoint) => {
                Err(crate::Error::UnexpectedEndpoint(format!("/{endpoint}")))
            }
        }
    }
}
