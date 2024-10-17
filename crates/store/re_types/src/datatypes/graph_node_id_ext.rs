use super::GraphNodeId;

impl std::convert::From<&str> for GraphNodeId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl std::convert::From<String> for GraphNodeId {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for GraphNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
