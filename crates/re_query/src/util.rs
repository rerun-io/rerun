use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_data_store::ExtraQueryHistory;
use re_log_types::{msg_bundle::Component, ComponentName, ObjPath};

use crate::{query_entity_with_primary, range_entity_with_primary, EntityView};

/// Either dispatch to `query_entity_with_primary` or `range_entity_with_primary`
/// depending on whether `ExtraQueryHistory` is set.
pub fn query_primary_with_history<'a, Primary: Component + 'a, const N: usize>(
    store: &'a DataStore,
    timeline: &'a Timeline,
    time: &'a TimeInt,
    history: &ExtraQueryHistory,
    ent_path: &'a ObjPath,
    components: [ComponentName; N],
) -> crate::Result<impl Iterator<Item = EntityView<Primary>> + 'a> {
    let visible_history = match timeline.typ() {
        re_log_types::TimeType::Time => history.nanos,
        re_log_types::TimeType::Sequence => history.sequences,
    };

    if visible_history == 0 {
        let latest_query = LatestAtQuery::new(*timeline, *time);
        let latest =
            query_entity_with_primary::<Primary>(store, &latest_query, ent_path, &components)?;

        Ok(itertools::Either::Left(std::iter::once(latest)))
    } else {
        let min_time = *time - TimeInt::from(visible_history);
        let range_query = RangeQuery::new(*timeline, TimeRange::new(min_time, *time));

        let range =
            range_entity_with_primary::<Primary, N>(store, &range_query, ent_path, components);

        Ok(itertools::Either::Right(range.map(|(_, entity)| entity)))
    }
}
