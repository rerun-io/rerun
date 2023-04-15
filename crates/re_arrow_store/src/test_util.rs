use crate::{DataStore, DataStoreConfig};

// ---

#[doc(hidden)]
#[macro_export]
macro_rules! test_row {
    ($entity:ident @ $frames:tt => $n:expr; [$c0:expr $(,)*]) => {{
        let mut row = ::re_log_types::DataRow::from_cells1(
            ::re_log_types::RowId::random(),
            $entity.clone(),
            $frames,
            $n,
            $c0,
        );
        row.compute_all_size_bytes();
        row
    }};
    ($entity:ident @ $frames:tt => $n:expr; [$c0:expr, $c1:expr $(,)*]) => {{
        let mut row = ::re_log_types::DataRow::from_cells2(
            ::re_log_types::RowId::random(),
            $entity.clone(),
            $frames,
            $n,
            ($c0, $c1),
        );
        row.compute_all_size_bytes();
        row
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

pub fn sanity_unwrap(store: &mut DataStore) {
    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }
}
