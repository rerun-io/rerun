use crate::Origin;

#[derive(Debug, PartialEq, Eq)]
pub struct ProxyEndpoint {
    pub origin: Origin,
}

impl std::fmt::Display for ProxyEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/catalog", self.origin)
    }
}

impl ProxyEndpoint {
    pub fn new(origin: Origin) -> Self {
        Self { origin }
    }
}
