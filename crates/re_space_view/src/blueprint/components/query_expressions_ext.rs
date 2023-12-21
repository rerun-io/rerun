use re_log_types::EntityPathFilter;

use super::QueryExpressions;

impl From<&EntityPathFilter> for QueryExpressions {
    fn from(filter: &EntityPathFilter) -> Self {
        Self(filter.formatted().into())
    }
}

impl From<&QueryExpressions> for EntityPathFilter {
    fn from(expressions: &QueryExpressions) -> Self {
        EntityPathFilter::parse_forgiving(&expressions.0)
    }
}
