impl std::convert::From<&str> for super::GraphNodeId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl std::convert::From<String> for super::GraphNodeId {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for super::GraphNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
