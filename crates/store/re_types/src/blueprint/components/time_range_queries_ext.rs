use re_log_types::TimelineName;

use super::TimeRangeQueries;
use crate::blueprint::datatypes::TimeRangeQuery;

impl TimeRangeQueries {
    /// Retrieves the query for a given timeline.
    pub fn query_for_timeline(&self, timeline_name: &TimelineName) -> Option<&TimeRangeQuery> {
        self.0
            .iter()
            .find(|query| query.timeline.as_str() == timeline_name.as_str())
    }

    /// Sets the query for a given timeline.
    pub fn set_query_for_timeline(&mut self, query: TimeRangeQuery) {
        if let Some(existing_query) = self.0.iter_mut().find(|q| q.timeline == query.timeline) {
            *existing_query = query;
        } else {
            self.0.push(query);
        }
    }
}
