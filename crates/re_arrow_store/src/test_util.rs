use crate::DataStoreConfig;

// ---

#[doc(hidden)]
#[macro_export]
macro_rules! test_row {
    ($entity:ident @ $frames:tt => $n:expr; [$c0:expr $(,)*]) => {
        ::re_log_types::DataRow::from_cells1(
            ::re_log_types::MsgId::random(),
            $entity.clone(),
            $frames,
            $n,
            $c0,
        )
    };
    ($entity:ident @ $frames:tt => $n:expr; [$c0:expr, $c1:expr $(,)*]) => {
        ::re_log_types::DataRow::from_cells2(
            ::re_log_types::MsgId::random(),
            $entity.clone(),
            $frames,
            $n,
            ($c0, $c1),
        )
    };
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
