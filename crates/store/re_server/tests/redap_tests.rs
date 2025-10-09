use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

fn build() -> RerunCloudHandler {
    RerunCloudHandlerBuilder::new().build()
}

re_redap_tests::generate_redap_tests!(build);
