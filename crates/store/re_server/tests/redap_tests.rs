use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

#[expect(clippy::unused_async)] // needed by the macro
async fn build() -> RerunCloudHandler {
    RerunCloudHandlerBuilder::new().build()
}

re_redap_tests::generate_redap_tests!(build);
