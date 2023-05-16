use re_arrow_store::LatestAtQuery;
use re_log_types::{
    DataRow, DeserializableComponent, EntityPath, RowId, SerializableComponent, TimeInt, TimePoint,
    Timeline,
};

use crate::LogDb;

// ----------------------------------------------------------------------------

/// Get the latest value for a given [`re_log_types::Component`].
///
/// This assumes that the row we get from the store only contains a single instance for this
/// component; it will log a warning otherwise.
///
/// This should only be used for "mono-components" such as `Transform` and `Tensor`.
pub fn query_latest_single<C: DeserializableComponent>(
    data_store: &re_arrow_store::DataStore,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<C>
where
    for<'b> &'b C::ArrayType: IntoIterator,
{
    crate::profile_function!();

    // Although it would be nice to use the `re_query` helpers for this, we would need to move
    // this out of re_data_store to avoid a circular dep. Since we don't need to do a join for
    // single components this is easy enough.

    let (_, cells) = data_store.latest_at(query, entity_path, C::name(), &[C::name()])?;
    let cell = cells.get(0)?.as_ref()?;

    let mut iter = cell.try_to_native::<C>().ok()?;

    let component = iter.next();

    if iter.next().is_some() {
        re_log::warn_once!("Unexpected batch for {} at: {}", C::name(), entity_path);
    }

    component
}

/// Get the latest value for a given [`re_log_types::Component`] assuming it is timeless.
///
/// This assumes that the row we get from the store only contains a single instance for this
/// component; it will log a warning otherwise.
pub fn query_timeless_single<C: DeserializableComponent>(
    data_store: &re_arrow_store::DataStore,
    entity_path: &EntityPath,
) -> Option<C>
where
    for<'b> &'b C::ArrayType: IntoIterator,
{
    let query = re_arrow_store::LatestAtQuery::new(Timeline::default(), TimeInt::MAX);
    query_latest_single(data_store, entity_path, &query)
}

// ----------------------------------------------------------------------------

/// Store a single value for a given [`re_log_types::Component`].
pub fn store_one_component<C: SerializableComponent>(
    log_db: &mut LogDb,
    entity_path: &EntityPath,
    timepoint: &TimePoint,
    component: C,
) {
    let mut row = DataRow::from_cells1(
        RowId::random(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component].as_slice(),
    );
    row.compute_all_size_bytes();

    match log_db.entity_db.try_add_data_row(&row) {
        Ok(()) => {}
        Err(err) => {
            re_log::warn_once!(
                "Failed to store component {}.{}: {err}",
                entity_path,
                C::name(),
            );
        }
    }
}
