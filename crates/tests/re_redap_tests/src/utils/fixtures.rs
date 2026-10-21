//! Datasets for the redap test suite, registered through [`RerunCloudService`], so each one is
//! set up the same way on every implementation: the OSS server and the Rerun Hub.

use re_log_types::TimeType;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;

use crate::tests::common::{DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _};

/// Registers three segments that share the entity `my/entity`; two of them add entities of their
/// own. The segments use `tuid_prefix` 1, 2, and 3, in order.
pub async fn register_simple_dataset(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> DataSourcesDefinition {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("my_segment_id1", &["my/entity", "my/other/entity"]),
            LayerDefinition::simple("my_segment_id2", &["my/entity"]),
            LayerDefinition::simple(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    create_and_register(service, dataset_name, data_sources_def).await
}

/// Registers three segments; two of them have a second layer, `extra`, with its own entity. The
/// layers use `tuid_prefix` 1 to 5, in registration order.
pub async fn register_layered_dataset(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> DataSourcesDefinition {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple("partition1", &["my/entity"]),
            LayerDefinition::simple("partition1", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("partition2", &["another/one"]).layer_name("base"),
            LayerDefinition::simple("partition2", &["extra/entity"]).layer_name("extra"),
            LayerDefinition::simple("partition3", &["i/am/alone"]),
        ],
    );

    create_and_register(service, dataset_name, data_sources_def).await
}

/// Registers two segments of the same entity: `segment_sequence` on a sequence timeline and
/// `segment_timestamp` on a timestamp timeline, with `tuid_prefix` 1 and 2.
pub async fn register_mixed_timelines_dataset(
    service: &impl RerunCloudService,
    dataset_name: &str,
) -> DataSourcesDefinition {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::simple_with_time(
                "segment_sequence",
                &["my/entity"],
                0,
                TimeType::Sequence,
            ),
            LayerDefinition::simple_with_time(
                "segment_timestamp",
                &["my/entity"],
                0,
                TimeType::TimestampNs,
            ),
        ],
    );

    create_and_register(service, dataset_name, data_sources_def).await
}

/// Registers three segments of `my/entity`, starting at 1000, 2000, and 3000 on `time_type`. Each
/// has four single-frame temporal chunks, at offsets 10, 20, 30, and 40, and one static chunk.
///
/// Hold the returned [`DataSourcesDefinition`] until the last query: the Hub re-reads the RRD files
/// lazily, and dropping them early surfaces as a `NotFound` error.
pub async fn register_per_segment_dataset(
    service: &impl RerunCloudService,
    dataset_name: &str,
    tuid_prefix: u64,
    time_type: TimeType,
) -> DataSourcesDefinition {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        tuid_prefix,
        [
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "seg1",
                &["my/entity"],
                1000,
                time_type,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "seg2",
                &["my/entity"],
                2000,
                time_type,
            ),
            LayerDefinition::simple_one_chunk_per_frame_with_time(
                "seg3",
                &["my/entity"],
                3000,
                time_type,
            ),
        ],
    );

    create_and_register(service, dataset_name, data_sources_def).await
}

/// Creates the dataset `dataset_name`, registers `data_sources_def` into it, and returns
/// `data_sources_def`, which owns the registered RRD files.
async fn create_and_register(
    service: &impl RerunCloudService,
    dataset_name: &str,
    data_sources_def: DataSourcesDefinition,
) -> DataSourcesDefinition {
    service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    data_sources_def
}
