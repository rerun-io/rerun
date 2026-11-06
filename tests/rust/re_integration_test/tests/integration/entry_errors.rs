use egui_kittest::kittest::Queryable as _;
use re_integration_test::{HarnessExt as _, TestServer};
use re_viewer::external::re_viewer_context::Route;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

#[tokio::test(flavor = "multi_thread")]
async fn missing_entry_shows_error_and_returns_to_server() {
    let (server, _) = TestServer::spawn().await.with_test_data().await;
    let origin = re_uri::Origin {
        host: re_uri::external::url::Host::Domain("localhost".to_owned()),
        port: server.port(),
        scheme: re_uri::Scheme::RerunHttp,
    };
    let missing_id = "00000000000000000000000000000001";

    for endpoint in ["dataset", "entry"] {
        let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
            startup_url: Some(format!(
                "rerun+http://localhost:{}/{endpoint}/{missing_id}",
                server.port()
            )),
            ..Default::default()
        });

        viewer_test_utils::step_until("Missing entry error appears", &mut harness, |harness| {
            harness.query_by_label("Could not open entry").is_some()
        });
        harness.get_by_label(&format!("Entry {missing_id} was not found on this server."));
        harness
            .get_by_role_and_label(egui::Role::Button, "Back to server")
            .click();

        let expected_route = Route::RedapServer(origin.clone());
        viewer_test_utils::step_until("Server page opens", &mut harness, |harness| {
            harness.state_mut().route() == &expected_route
        });
        let selection = harness.run_with_app_context(|ctx| ctx.selection().single_item().cloned());
        assert_eq!(selection, expected_route.item());
        assert!(harness.query_by_label("Could not open entry").is_none());
    }
}
