use re_log_types::hash::Hash64;

use super::GraphNode;

impl std::convert::From<&str> for GraphNode {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl std::convert::From<String> for GraphNode {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for GraphNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
