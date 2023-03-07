use crate::DataStoreConfig;

// ---

#[doc(hidden)]
#[macro_export]
macro_rules! test_bundle {
    ($entity:ident @ $frames:tt => [$c0:expr $(,)*]) => {
        ::re_log_types::msg_bundle::try_build_msg_bundle1(
            ::re_log_types::MsgId::random(),
            $entity.clone(),
            $frames,
            $c0,
        )
        .unwrap()
    };
    ($entity:ident @ $frames:tt => [$c0:expr, $c1:expr $(,)*]) => {
        re_log_types::msg_bundle::try_build_msg_bundle2(
            ::re_log_types::MsgId::random(),
            $entity.clone(),
            $frames,
            ($c0, $c1),
        )
        .unwrap()
    };
}

pub fn all_configs() -> impl Iterator<Item = DataStoreConfig> {
    const COMPONENT_CONFIGS: &[DataStoreConfig] = &[
        DataStoreConfig::DEFAULT,
        // DataStoreConfig {
        //     component_bucket_nb_rows: 0,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_nb_rows: 1,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_nb_rows: 2,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_nb_rows: 3,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_size_bytes: 0,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_size_bytes: 16,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_size_bytes: 32,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     component_bucket_size_bytes: 64,
        //     ..DataStoreConfig::DEFAULT
        // },
    ];

    const INDEX_CONFIGS: &[DataStoreConfig] = &[
        DataStoreConfig::DEFAULT,
        // DataStoreConfig {
        //     index_bucket_nb_rows: 0,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_nb_rows: 1,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_nb_rows: 2,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_nb_rows: 3,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_size_bytes: 0,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_size_bytes: 16,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_size_bytes: 32,
        //     ..DataStoreConfig::DEFAULT
        // },
        // DataStoreConfig {
        //     index_bucket_size_bytes: 64,
        //     ..DataStoreConfig::DEFAULT
        // },
    ];
    COMPONENT_CONFIGS.iter().flat_map(|comp| {
        INDEX_CONFIGS.iter().map(|idx| DataStoreConfig {
            component_bucket_size_bytes: comp.component_bucket_size_bytes,
            component_bucket_nb_rows: comp.component_bucket_nb_rows,
            index_bucket_size_bytes: idx.index_bucket_size_bytes,
            index_bucket_nb_rows: idx.index_bucket_nb_rows,
            store_insert_ids: comp.store_insert_ids || idx.store_insert_ids,
        })
    })
}
