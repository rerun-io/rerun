use re_log_types::DataTable;

use crate::{DataStore, DataStoreConfig, WriteError};

// ---

#[doc(hidden)]
#[macro_export]
macro_rules! test_row {
    ($entity:ident => $n:expr; [$c0:expr $(,)*]) => {{
        ::re_log_types::DataRow::from_cells1_sized(
            ::re_log_types::RowId::new(),
            $entity.clone(),
            ::re_log_types::TimePoint::timeless(),
            $n,
            $c0,
        )
        .unwrap()
    }};
    ($entity:ident @ $frames:tt => $n:expr; [$c0:expr $(,)*]) => {{
        ::re_log_types::DataRow::from_cells1_sized(
            ::re_log_types::RowId::new(),
            $entity.clone(),
            $frames,
            $n,
            $c0,
        )
        .unwrap()
    }};
    ($entity:ident @ $frames:tt => $n:expr; [$c0:expr, $c1:expr $(,)*]) => {{
        ::re_log_types::DataRow::from_cells2_sized(
            ::re_log_types::RowId::new(),
            $entity.clone(),
            $frames,
            $n,
            ($c0, $c1),
        )
        .unwrap()
    }};
}

pub fn all_configs() -> impl Iterator<Item = DataStoreConfig> {
    const INDEX_CONFIGS: &[DataStoreConfig] = &[
        DataStoreConfig::DEFAULT,
        DataStoreConfig {
            indexed_bucket_num_rows: 0,
            ..DataStoreConfig::DEFAULT
        },
        DataStoreConfig {
            indexed_bucket_num_rows: 1,
            ..DataStoreConfig::DEFAULT
        },
        DataStoreConfig {
            indexed_bucket_num_rows: 2,
            ..DataStoreConfig::DEFAULT
        },
        DataStoreConfig {
            indexed_bucket_num_rows: 3,
            ..DataStoreConfig::DEFAULT
        },
    ];
    INDEX_CONFIGS.iter().map(|idx| DataStoreConfig {
        indexed_bucket_num_rows: idx.indexed_bucket_num_rows,
        store_insert_ids: idx.store_insert_ids,
        enable_typecheck: idx.enable_typecheck,
    })
}

pub fn sanity_unwrap(store: &DataStore) {
    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }
}

// We very often re-use RowIds when generating test data.
pub fn insert_table_with_retries(store: &mut DataStore, table: &DataTable) {
    for row in table.to_rows() {
        let mut row = row.unwrap();
        loop {
            match store.insert_row(&row) {
                Ok(_) => break,
                Err(WriteError::ReusedRowId(_)) => {
                    row.row_id = row.row_id.next();
                }
                err @ Err(_) => err.map(|_| ()).unwrap(),
            }
        }
    }
}
