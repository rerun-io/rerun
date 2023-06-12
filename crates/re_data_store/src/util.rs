use re_log_types::{DataRow, EntityPath, RowId, SerializableComponent, TimePoint};

use crate::StoreDb;

// ----------------------------------------------------------------------------

/// Store a single value for a given [`re_log_types::Component`].
pub fn store_one_component<C: SerializableComponent>(
    store_db: &mut StoreDb,
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
