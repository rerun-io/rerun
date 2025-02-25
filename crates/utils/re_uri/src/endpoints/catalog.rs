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
