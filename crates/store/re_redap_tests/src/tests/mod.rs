mod column_projection;
mod common;
mod create_dataset;
mod create_table;
mod dataset_schema;
mod entries_table;
mod fetch_chunks;
mod indexes;
mod query_dataset;
mod query_filter;
mod register_segment;
mod rrd_manifest;
mod update_entry;
mod write_table;

macro_rules! define_redap_tests {
    (
        $(
            $mod:ident :: $test:ident
        ),* $(,)?
    ) => {
        // Generate public wrapper functions
        //
        // The purpose of these wrappers is to allow the _actual_ tests to be not be exported by
        // this crate. As a result, the `dead_code` lint will kick in one forgets to add them to the
        // definition below.
        $(
            pub async fn $test<T>(service: T)
            where
                T: re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
            {
                $mod::$test(service).await;
            }
        )*

        // Generate the test instantiation macro
        //
        // This is the macro that must be used to actually instantiate the tests in implementing
        // crates/repos.
        #[macro_export]
        macro_rules! generate_redap_tests {
            ($builder:ident) => {
                $(
                    #[tokio::test]
                    async fn $test() {
                        $crate::$test($builder().await).await
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
    dataset_schema::empty_dataset_schema,
    dataset_schema::simple_dataset_schema,
    entries_table::entries_table_with_empty_dataset,
    entries_table::list_entries_table,
    fetch_chunks::multi_dataset_fetch_chunk_completeness,
    fetch_chunks::simple_dataset_fetch_chunk_snapshot,
    indexes::column_doesnt_exist,
    indexes::dataset_doesnt_exist,
    indexes::index_lifecycle,
    query_dataset::query_dataset_should_fail,
    query_dataset::query_empty_dataset,
    query_dataset::query_simple_dataset,
    query_dataset::query_simple_dataset_with_layers,
    query_dataset::query_dataset_with_various_queries,
    query_filter::query_dataset_simple_filter,
    register_segment::register_and_scan_blueprint_dataset,
    register_segment::register_and_scan_empty_dataset,
    register_segment::register_and_scan_simple_dataset,
    register_segment::register_and_scan_simple_dataset_with_layers,
    register_segment::register_and_scan_simple_dataset_with_properties,
    register_segment::register_and_scan_simple_dataset_with_properties_out_of_order,
    register_segment::register_and_scan_simple_dataset_multiple_timelines,
    register_segment::register_bad_file_uri_should_error,
    register_segment::register_segment_bumps_timestamp,
    register_segment::register_with_prefix,
    rrd_manifest::segment_id_not_found,
    rrd_manifest::simple_dataset_rrd_manifest,
    update_entry::update_entry_bumps_timestamp,
    update_entry::update_entry_tests,
    write_table::write_table
}
