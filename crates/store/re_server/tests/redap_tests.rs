use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

async fn build() -> RerunCloudHandler {
    RerunCloudHandlerBuilder::new().build()
}

re_redap_tests::generate_redap_tests!(build);
