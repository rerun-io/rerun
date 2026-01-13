#![cfg(feature = "lance")]

use re_server::{RerunCloudHandler, RerunCloudHandlerBuilder};

#[expect(clippy::unused_async)] // needed by the macro
async fn build() -> RerunCloudHandler {
    RerunCloudHandlerBuilder::new().build()
}

re_redap_tests::generate_redap_tests!(build);

#[tokio::test(flavor = "multi_thread")]
async fn version() {
    let (handle, addr) = re_server::Args {
        host: "127.0.0.1".into(),
        port: 0,
        datasets: vec![],
        dataset_prefixes: vec![],
        tables: vec![],
    }
    .create_server_handle()
    .await
    .expect("failed to start server");

    let response = ehttp::fetch_async(ehttp::Request::get(format!("http://{addr}/version")))
        .await
        .expect("failed to get `/version`");
    let text = response.text();
    if !response.ok {
        eprintln!(
            "failed to get `/version`, error: {} {} {text:?}",
            response.status, response.status_text
        );
        handle.shutdown_and_wait().await;
        panic!();
    }

    assert_eq!(
        text,
        Some(re_build_info::build_info!().to_string().as_str())
    );

    handle.shutdown_and_wait().await;
}
