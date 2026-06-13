use egui_kittest::kittest::Queryable as _;
use re_sdk_types::blueprint::archetypes::WebPageViewConfig;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_web_page::WebPageView;
use re_view_web_page::testing::FakeWebViewBackend;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewerContext};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty, ViewportBlueprint};

#[test]
fn manually_created_web_page_view_without_url_shows_status() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            WebPageView::identifier(),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(
        harness
            .query_by_label_contains("No URL configured")
            .is_some(),
        "expected Web Page View to explain that no URL is configured"
    );
}

#[test]
fn blueprint_configured_web_page_view_reads_url_and_navigation_preference_without_logged_data() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();

    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", false);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(
        harness
            .query_by_label_contains("https://example.com")
            .is_some(),
        "expected Web Page View to read and display the configured URL"
    );
    assert!(
        harness
            .query_by_label_contains("No URL configured")
            .is_none(),
        "expected configured Web Page View not to render the missing-URL status"
    );
    assert!(
        harness.query_by_label_contains("Back").is_none(),
        "expected navigation controls to be hidden when show_navigation_controls is false"
    );
}

#[test]
fn https_url_is_accepted() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert_eq!(
        harness
            .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address")
            .value()
            .as_deref(),
        Some("https://example.com")
    );
    assert!(harness.query_by_label_contains("Invalid URL").is_none());
    assert!(
        harness
            .query_by_label_contains("Unsupported URL scheme")
            .is_none()
    );
}

#[test]
fn localhost_http_url_is_accepted() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "http://localhost:3000", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert_eq!(
        harness
            .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address")
            .value()
            .as_deref(),
        Some("http://localhost:3000")
    );
    assert!(harness.query_by_label_contains("Invalid URL").is_none());
    assert!(
        harness
            .query_by_label_contains("Unsupported URL scheme")
            .is_none()
    );
}

#[test]
fn navigation_controls_are_visible_by_default() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_web_page_view_with_default_navigation_controls(
        &mut test_context,
        "https://example.com",
    );

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(harness.query_by_label_contains("Back").is_some());
    assert!(harness.query_by_label_contains("Forward").is_some());
    assert!(harness.query_by_label_contains("Reload").is_some());
    assert!(harness.query_by_label_contains("Home").is_some());
}

#[test]
fn navigation_controls_can_be_hidden() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", false);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(harness.query_by_label_contains("Back").is_none());
    assert!(harness.query_by_label_contains("Forward").is_none());
    assert!(harness.query_by_label_contains("Reload").is_none());
    assert!(harness.query_by_label_contains("Home").is_none());
}

#[test]
fn home_navigation_does_not_mutate_configured_url() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let configured_url = "https://example.com/home";
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, configured_url, true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();

    fake_backend.simulate_navigation(view_id, "https://example.com/runtime");
    harness.get_by_label("Home").click();
    harness.run();

    let navigation_requests = fake_backend.navigation_requests();
    assert_eq!(navigation_requests.len(), 1);
    assert_eq!(navigation_requests[0].view_id, view_id);
    assert_eq!(navigation_requests[0].url, configured_url);
    assert_eq!(
        harness
            .get_by_role_and_label(egui::accesskit::Role::TextInput, "Address")
            .value()
            .as_deref(),
        Some(configured_url)
    );
}

#[test]
fn navigation_controls_include_editable_address_bar() {
    let configured_url = "https://example.com/home";
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, configured_url, true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([700.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();

    let address_bar = harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Address");
    assert_eq!(address_bar.value().as_deref(), Some(configured_url));
    assert!(harness.query_by_label("Go").is_some());
}

#[test]
fn file_url_shows_unsupported_scheme_status() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id =
        setup_configured_web_page_view(&mut test_context, "file:///tmp/report.html", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(
        harness
            .query_by_label_contains("Unsupported URL scheme")
            .is_some(),
        "expected file URLs to be rejected by Rerun-side status UI"
    );
}

#[test]
fn invalid_url_text_shows_invalid_status() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "not a url", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(
        harness.query_by_label_contains("Invalid URL").is_some(),
        "expected invalid URL text to be rejected by Rerun-side status UI"
    );
}

#[test]
fn unavailable_native_backend_shows_status_outside_webview() {
    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(
        harness
            .query_by_label_contains("Embedded webview unavailable")
            .is_some(),
        "expected explicit status when no native backend is available"
    );
}

#[test]
fn backend_creation_failure_shows_status_outside_webview() {
    let fake_backend = FakeWebViewBackend::failing("backend failed");
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert!(
        harness
            .query_by_label_contains("Failed to create embedded webview")
            .is_some(),
        "expected explicit status when backend creation fails"
    );
    assert!(harness.query_by_label_contains("backend failed").is_some());
}

#[test]
fn valid_configured_url_creates_one_backend_webview_instance() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    assert_eq!(fake_backend.created_instance_count(), 1);
    assert_eq!(fake_backend.created_urls(), ["https://example.com"]);
}

#[test]
fn two_web_page_views_create_independent_backend_webview_instances() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let (first_view_id, second_view_id) =
        test_context.setup_viewport_blueprint(|ctx, blueprint| {
            let first_view_id =
                add_configured_web_page_view(ctx, blueprint, "https://example.com/first", true);
            let second_view_id =
                add_configured_web_page_view(ctx, blueprint, "https://example.com/second", true);
            (first_view_id, second_view_id)
        });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 500.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, first_view_id);
            test_context.run_with_single_view(ui, second_view_id);
        });

    harness.run();

    let created_instances = fake_backend.created_instances();
    assert_eq!(created_instances.len(), 2);
    assert_eq!(created_instances[0].view_id, first_view_id);
    assert_eq!(created_instances[0].url, "https://example.com/first");
    assert_eq!(created_instances[1].view_id, second_view_id);
    assert_eq!(created_instances[1].url, "https://example.com/second");
}

#[test]
fn multiple_web_page_views_use_shared_default_browser_session() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let (first_view_id, second_view_id) =
        test_context.setup_viewport_blueprint(|ctx, blueprint| {
            let first_view_id =
                add_configured_web_page_view(ctx, blueprint, "https://example.com/first", true);
            let second_view_id =
                add_configured_web_page_view(ctx, blueprint, "https://example.com/second", true);
            (first_view_id, second_view_id)
        });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 500.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, first_view_id);
            test_context.run_with_single_view(ui, second_view_id);
        });

    harness.run();

    let created_instances = fake_backend.created_instances();
    assert_eq!(created_instances.len(), 2);
    assert_eq!(created_instances[0].session, created_instances[1].session);
    assert_eq!(created_instances[0].session.as_str(), "shared-default");
}

#[test]
fn backend_receives_updated_bounds_when_view_rect_changes() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut first_harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    first_harness.run();

    let mut second_harness = test_context
        .setup_kittest_for_rendering_ui([700.0, 350.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    second_harness.run();

    let bounds_updates = fake_backend.bounds_updates();
    assert!(
        bounds_updates.len() >= 2,
        "expected bounds updates from both rendered view sizes"
    );
    assert!(
        bounds_updates
            .iter()
            .all(|bounds_update| bounds_update.view_id == view_id)
    );
    assert_ne!(
        bounds_updates.first().unwrap().bounds.size,
        bounds_updates.last().unwrap().bounds.size
    );
}

#[test]
fn hidden_web_page_view_keeps_backend_instance_alive() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();

    assert_eq!(fake_backend.created_instance_count(), 1);

    // Simulate a hidden tab by not rendering the view for a frame. The view state remains owned by
    // the viewer and therefore must keep its native webview alive.
    let mut hidden_harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|_ui| {});
    hidden_harness.run();

    assert_eq!(fake_backend.destroyed_instance_count(), 0);
}

#[test]
fn removed_web_page_view_destroys_backend_instance() {
    let fake_backend = FakeWebViewBackend::default();
    let _backend_guard = fake_backend.install();

    let mut test_context = TestContext::new_with_view_class::<WebPageView>();
    let view_id = setup_configured_web_page_view(&mut test_context, "https://example.com", true);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([500.0, 250.0])
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();

    assert_eq!(fake_backend.created_instance_count(), 1);

    test_context
        .view_states
        .lock()
        .retain_for_views(&test_context.recording_store_id, []);

    assert_eq!(fake_backend.destroyed_instance_count(), 1);
}

fn setup_configured_web_page_view(
    test_context: &mut TestContext,
    url: &str,
    show_navigation_controls: bool,
) -> re_viewer_context::ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        add_configured_web_page_view(ctx, blueprint, url, show_navigation_controls)
    })
}

fn setup_web_page_view_with_default_navigation_controls(
    test_context: &mut TestContext,
    url: &str,
) -> re_viewer_context::ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(WebPageView::identifier());
        let config = ViewProperty::from_archetype::<WebPageViewConfig>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view.id,
        );

        ctx.save_blueprint_archetype(config.blueprint_store_path, &WebPageViewConfig::new(url));

        blueprint.add_view_at_root(view)
    })
}

fn add_configured_web_page_view(
    ctx: &ViewerContext<'_>,
    blueprint: &mut ViewportBlueprint,
    url: &str,
    show_navigation_controls: bool,
) -> re_viewer_context::ViewId {
    let view = ViewBlueprint::new_with_root_wildcard(WebPageView::identifier());
    let config = ViewProperty::from_archetype::<WebPageViewConfig>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        view.id,
    );

    ctx.save_blueprint_archetype(
        config.blueprint_store_path,
        &WebPageViewConfig::new(url).with_show_navigation_controls(show_navigation_controls),
    );

    blueprint.add_view_at_root(view)
}
