use re_data_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_entity_db::ExtraQueryHistory;
use re_log_types::EntityPath;
use re_types_core::Archetype;

use crate::{query_archetype, range::range_archetype, ArchetypeView};

pub fn query_archetype_with_history<'a, A: Archetype + 'a, const N: usize>(
    store: &'a DataStore,
    timeline: &'a Timeline,
    time: &'a TimeInt,
    history: &ExtraQueryHistory,
    ent_path: &'a EntityPath,
) -> crate::Result<impl Iterator<Item = ArchetypeView<A>> + 'a> {
    let visible_history = match timeline.typ() {
        re_log_types::TimeType::Time => history.nanos,
        re_log_types::TimeType::Sequence => history.sequences,
    };

    let min_time = visible_history.from(*time);
    let max_time = visible_history.to(*time);

    if !history.enabled || min_time == max_time {
        let latest_query = LatestAtQuery::new(*timeline, min_time);
        let latest = query_archetype::<A>(store, &latest_query, ent_path)?;

        Ok(itertools::Either::Left(std::iter::once(latest)))
    } else {
        let range_query = RangeQuery::new(*timeline, TimeRange::new(min_time, max_time));

        let range = range_archetype::<A, N>(store, &range_query, ent_path);

        Ok(itertools::Either::Right(range.map(|(_, entity)| entity)))
    }
}
