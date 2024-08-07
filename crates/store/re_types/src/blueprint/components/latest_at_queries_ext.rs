use super::LatestAtQueries;
use crate::blueprint::datatypes::LatestAtQuery;

impl LatestAtQueries {
    /// Retrieves the query for a given timeline.
    pub fn query_for_timeline(&self, timeline_name: &str) -> Option<&LatestAtQuery> {
        self.0
            .iter()
            .find(|query| query.timeline.as_str() == timeline_name)
    }

    /// Sets the query for a given timeline.
    pub fn set_query_for_timeline(&mut self, query: LatestAtQuery) {
        if let Some(existing_query) = self.0.iter_mut().find(|q| q.timeline == query.timeline) {
            *existing_query = query;
        } else {
            self.0.push(query);
        }
    }
}
