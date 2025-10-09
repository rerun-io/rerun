mod entries_table;

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
            pub async fn $test<T>(builder: impl FnOnce() -> T)
            where
                T: re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService,
            {
                $mod::$test(builder()).await;
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
                        $crate::$test(|| $builder()).await
                    }
                )*
            };
        }
    };
}

define_redap_tests! {
    entries_table::list_entries_table,
}
