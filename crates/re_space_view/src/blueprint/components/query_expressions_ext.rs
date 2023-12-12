use re_log_types::EntityPathExpr;

use super::QueryExpressions;

impl QueryExpressions {
    pub fn new(
        inclusions: impl Iterator<Item = EntityPathExpr>,
        exclusions: impl Iterator<Item = EntityPathExpr>,
    ) -> Self {
        Self(crate::blueprint::datatypes::QueryExpressions {
            inclusions: inclusions
                .into_iter()
                .map(|s| s.to_string().into())
                .collect(),
            exclusions: exclusions
                .into_iter()
                .map(|s| s.to_string().into())
                .collect(),
        })
    }
}
