mod common;
mod dataset_schema;
mod entries_table;
mod fetch_chunks;
mod query_dataset;
mod register_partition;

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
    dataset_schema::empty_dataset_schema,
    dataset_schema::simple_dataset_schema,
    entries_table::list_entries_table,
    fetch_chunks::simple_dataset_fetch_chunk_snapshot,
    query_dataset::query_dataset_should_fail,
    query_dataset::query_empty_dataset,
    query_dataset::query_simple_dataset,
    register_partition::register_and_scan_empty_dataset,
    register_partition::register_and_scan_simple_dataset,
    register_partition::register_and_scan_simple_dataset_with_layers,
}
