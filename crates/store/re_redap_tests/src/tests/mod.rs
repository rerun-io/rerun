mod column_projection;
pub mod common;
mod create_dataset;
mod create_table;
mod dataset_schema;
mod entries_table;
mod fetch_chunks;
mod query_dataset;
mod query_filter;
mod query_index_values;
mod register_asset;
mod register_segment;
mod rrd_manifest;
mod unregister_segment;
mod update_entry;
mod watch_events;
mod write_table;

/// Generate wrappers and the `generate_redap_tests!`/`generate_oss_only_redap_tests!`
/// instantiation macros.
///
/// Takes three semicolon-separated lists:
///   - First list: tests whose bodies return `()`.
///   - Second list: tests whose bodies return `anyhow::Result<()>` (wrapped with `.expect`).
///   - Third list: like the second, but only instantiated by `generate_oss_only_redap_tests!`,
///     for features the cloud server does not implement yet.
///
/// The `dead_code` lint fires if a test is accidentally omitted from all lists.
macro_rules! define_redap_tests {
    (
        $( $mod:ident :: $test:ident ),* $(,)?
        ;
        $( $rmod:ident :: $rtest:ident ),* $(,)?
        ;
        $( $omod:ident :: $otest:ident ),* $(,)?
    ) => {
        $(
            pub async fn $test<T>(service: T)
            where
                T: re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
            {
                $mod::$test(service).await;
            }
        )*

        $(
            pub async fn $rtest<T>(service: T)
            where
                T: re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
            {
                $rmod::$rtest(service).await.expect(stringify!($rtest));
            }
        )*

        $(
            pub async fn $otest<T>(service: T)
            where
                T: re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
            {
                $omod::$otest(service).await.expect(stringify!($otest));
            }
        )*

        #[macro_export]
        macro_rules! generate_redap_tests {
            ($builder:ident) => {
                $(
                    #[tokio::test]
                    async fn $test() {
                        $crate::$test($builder().await).await
                    }
                )*
                $(
                    #[tokio::test]
                    async fn $rtest() {
                        $crate::$rtest($builder().await).await
                    }
                )*
            };
        }

        /// Tests for features only implemented by the OSS server (`re_server`) so far.
        #[macro_export]
        macro_rules! generate_oss_only_redap_tests {
            ($builder:ident) => {
                $(
                    #[tokio::test]
                    async fn $otest() {
                        $crate::$otest($builder().await).await
                    }
                )*
            };
        }
    };
}

define_redap_tests! {
    column_projection::test_dataset_manifest_column_projections,
    column_projection::test_segment_table_column_projections,
    create_dataset::create_dataset_tests,
    create_table::create_table_entry,
    create_table::create_table_entry_duplicate_url,
    create_table::create_table_entry_failed_does_not_leak_name,
    dataset_schema::empty_dataset_schema,
    dataset_schema::simple_dataset_schema,
    entries_table::delete_table_deletes_attached_blueprint_dataset,
    entries_table::entries_table_with_empty_dataset,
    entries_table::list_entries_table,
    fetch_chunks::multi_dataset_fetch_chunk_completeness,
    fetch_chunks::simple_dataset_fetch_chunk_snapshot,
    query_dataset::query_dataset_should_fail,
    query_dataset::query_dataset_unknown_segment_id_returns_empty,
    query_dataset::query_dataset_consistent_schema_across_timelines,
    query_dataset::query_dataset_has_uncompressed_sizes,
    query_dataset::query_dataset_with_various_queries,
    query_dataset::query_empty_dataset,
    query_dataset::query_simple_dataset,
    query_dataset::query_simple_dataset_with_layers,
    query_filter::query_dataset_range_filter_with_and_without_latest_at_fill,
    query_filter::query_dataset_simple_filter,
    query_filter::query_dataset_with_limit,
    query_index_values::query_dataset_emits_per_segment_pushdown,
    query_index_values::query_dataset_index_values,
    query_index_values::query_dataset_per_segment_values_wire_level,
    query_index_values::query_dataset_per_segment_values_multi_value_wire_level,
    query_index_values::query_dataset_per_segment_values_validation_rejected,
    query_index_values::query_dataset_per_segment_values_with_chunk_ids_intersects,
    query_index_values::query_dataset_per_segment_values_empty_entity_paths_short_circuits,
    register_asset::asset_dataset_enforces_segment_limit,
    register_asset::asset_dataset_enforces_segment_size_limit,
    register_asset::asset_dataset_rejects_temporal_recording,
    register_asset::blueprint_dataset_enforces_segment_size_limit,
    register_asset::deleting_dataset_deletes_asset_dataset,
    register_asset::get_assets_for_segment_rejects_non_recording_dataset,
    register_asset::get_assets_for_segment_returns_registered_assets,
    register_segment::register_and_attach_table_blueprint_dataset,
    register_segment::register_and_scan_blueprint_dataset,
    register_segment::register_and_scan_empty_dataset,
    register_segment::register_and_scan_simple_dataset,
    register_segment::register_and_scan_simple_dataset_multiple_timelines,
    register_segment::register_and_scan_simple_dataset_with_layers,
    register_segment::register_and_scan_simple_dataset_with_properties,
    register_segment::register_and_scan_simple_dataset_with_properties_out_of_order,
    register_segment::register_bad_file_uri_should_error,
    register_segment::register_conflicting_property_schema,
    register_segment::register_conflicting_schema,
    register_segment::register_conflicting_schema_filters_segment_table,
    register_segment::register_conflicting_schema_same_segment_filters_layer,
    register_segment::register_empty_request,
    register_segment::register_fully_skipped,
    register_segment::register_intra_request_duplicates,
    register_segment::register_segment_bumps_timestamp,
    register_segment::register_with_dataset_if_duplicate_behavior_error,
    register_segment::register_with_dataset_if_duplicate_behavior_overwrite,
    register_segment::register_with_dataset_if_duplicate_behavior_skip,
    register_segment::register_with_prefix,
    rrd_manifest::segment_id_not_found,
    rrd_manifest::simple_dataset_rrd_manifest,
    rrd_manifest::unregistered_segment,
    rrd_manifest::layered_segment,
    rrd_manifest::layered_segment_stress,
    unregister_segment::unregister_invalid_args,
    unregister_segment::unregister_missing_dataset,
    unregister_segment::unregister_missing_segment,
    unregister_segment::unregister_products,
    unregister_segment::unregister_simple,
    unregister_segment::unregister_then_query,
    update_entry::update_dataset_entry_keeps_asset_dataset,
    update_entry::update_dataset_entry_rejects_invalid_asset_details,
    update_entry::update_dataset_entry_rejects_invalid_blueprint_details,
    update_entry::update_dataset_entry_replaces_deleted_asset_dataset,
    update_entry::update_entry_bumps_timestamp,
    update_entry::update_entry_tests,
    update_entry::update_table_entry_blueprint_details,
    update_entry::update_table_entry_rejects_invalid_blueprint_details,
    write_table::write_table,
    ; // Tests that return `anyhow::Result<()>`:
    ; // OSS-only tests:
    // TODO(RR-4859): the cloud server returns `Unimplemented` for `WatchEvents`.
    watch_events::watch_events_entry_created,
    watch_events::watch_events_entry_deleted,
}
