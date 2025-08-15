use re_recording_panel::data::RecordingPanelData;
use re_test_context::TestContext;

#[test]
fn empty_context_test() {
    let test_context = TestContext::new();
    let servers = re_redap_browser::RedapServers::default();

    test_context.run_once_in_egui_central_panel(|ctx, _| {
        let data = RecordingPanelData::new(ctx, &servers, false);

        insta::assert_yaml_snapshot!(data);
    });
}

//TODO(ab): we need more tests here.
