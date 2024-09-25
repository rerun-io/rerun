impl std::convert::From<&str> for super::GraphNodeId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}
