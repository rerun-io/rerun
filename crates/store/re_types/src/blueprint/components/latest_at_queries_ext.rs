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
    ///
    /// If the query is `None`, the timeline will be removed from the list of latest queries.
    pub fn set_query_for_timeline(&mut self, timeline_name: &str, query: Option<LatestAtQuery>) {
        if let Some(query) = query {
            if let Some(existing_query) = self
                .0
                .iter_mut()
                .find(|query| query.timeline.as_str() == timeline_name)
            {
                *existing_query = query;
            } else {
                self.0.push(query);
            }
        } else {
            self.0.retain(|q| q.timeline.as_str() != timeline_name);
        }
    }
}
