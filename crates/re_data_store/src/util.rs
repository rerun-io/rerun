use re_log_types::{DataRow, EntityPath, RowId, TimePoint};
use re_types::Component;

use crate::StoreDb;

// ----------------------------------------------------------------------------

/// Store a single value for a given [`Component`].
///
/// BEWARE: This does more than just writing component data to the datastore, it actually updates
/// several other datastructures in the process.
/// This is _not_ equivalent to [`re_arrow_store::DataStore::insert_component`]!
pub fn store_one_component<'a, C>(
    store_db: &mut StoreDb,
    entity_path: &EntityPath,
    timepoint: &TimePoint,
    component: C,
) where
    C: Component + Clone + 'a,
    C: Into<::std::borrow::Cow<'a, C>>,
{
    let mut row = DataRow::try_from_cells1(
        RowId::random(),
        entity_path.clone(),
        timepoint.clone(),
        1,
        [component],
    )
    .unwrap();
    row.compute_all_size_bytes();

    match store_db.entity_db.try_add_data_row(&row) {
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
