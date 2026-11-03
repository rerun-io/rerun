use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use re_ui::notifications::{Notification, NotificationLevel, NotificationUi};

/// End-to-end test: a single `re_log::warn!` call carrying a string field, an integer field
/// and a message should turn into a toast with each `key: value` on its own line.
#[test]
fn test_notification_with_fields() {
    // Register a receiver before emitting, so the channel logger captures our event.
    let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::INFO);

    // Installs the global tracing subscriber (including the channel logger) once.
    re_log::setup_logging();

    // `target: "re_ui"` so it passes the notification relevance filter (rerun-crate + WARN).
    re_log::warn!(
        target: "re_ui",
        user_name = "bob",
        num_attempts = 42,
        "Failed to connect"
    );

    let log_msg = log_rx
        .try_recv()
        .expect("the channel logger should have captured the warning");

    let mut notifications: Option<NotificationUi> = None;

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(400.0, 200.0))
            .build_ui(move |ui| {
                let notifications = notifications.get_or_insert_with(|| {
                    let mut notifications = NotificationUi::new(ui.ctx().clone());
                    notifications.add_log(log_msg.clone());
                    notifications
                });

                notifications.show_toasts(ui.ctx());
            });

    harness.run();
    harness.snapshot("notification_with_fields");

    // The fields are details like any other, one click away:
    harness.get_by_label("Details").click();
    harness.run();
    harness.snapshot("notification_with_fields_expanded");
}

#[test]
fn test_notification_with_urls() {
    let mut notifications: Option<NotificationUi> = None;

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(400.0, 220.0))
            .build_ui(move |ui| {

                let notifications = notifications.get_or_insert_with(|| {
                    let mut notifications = NotificationUi::new(ui.ctx().clone());
                    notifications.add(Notification::new(
                        NotificationLevel::Warning,
                        "Failed to load https://rerun.invalid/docs. Check http://example.invalid/status or mailto:help@example.invalid for updates.",
                    ));
                    notifications
                });

                notifications.show_toasts(ui.ctx());
            });

    harness.run();
    harness.snapshot("notification_with_urls");
}

/// A message with details (see [`re_error::StructuredError`]) should have those moved into the
/// collapsible details section instead of being shown inline.
#[test]
fn test_notification_with_details_in_field() {
    let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::INFO);
    re_log::setup_logging();

    // A real server error: a message with details of its own, plus a trace-id and response
    // metadata, wrapped in the `ApiError` the client hands to the viewer.
    let mut status =
        tonic::Status::internal("invalid lance input\n- dataset url: file:///path/to/file");
    status.metadata_mut().insert(
        "x-request-trace-id",
        tonic::metadata::MetadataValue::from_static("ad66019921fce81f3f56462f9a8dbd63"),
    );
    let err = re_redap_client::ApiError::tonic(&re_uri::Origin::test(), status, "/GetTableSchema");

    // `target: "re_ui"` so it passes the notification relevance filter (rerun-crate + WARN).
    re_log::error!(target: "re_ui", "An error occurred: {err}");

    let log_msg = log_rx
        .try_recv()
        .expect("the channel logger should have captured the error");

    let mut notifications: Option<NotificationUi> = None;

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(520.0, 280.0))
            .build_ui(move |ui| {
                let notifications = notifications.get_or_insert_with(|| {
                    let mut notifications = NotificationUi::new(ui.ctx().clone());
                    notifications.add_log(log_msg.clone());
                    notifications
                });

                notifications.show_toasts(ui.ctx());
            });

    harness.run();
    harness.snapshot("notification_with_details_in_field");

    // The details are one click away, and the toast widens to fit them:
    harness.get_by_label("Details").click();
    harness.run();
    harness.snapshot("notification_with_details_in_field_expanded");
}

/// A failure that repeats shows up as one toast with a count, with the details the occurrences
/// share shown once, and the rest grouped per occurrence.
#[test]
fn test_repeated_notification() {
    let mut notifications: Option<NotificationUi> = None;

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(520.0, 420.0))
            .build_ui(move |ui| {
                let notifications = notifications.get_or_insert_with(|| {
                    let mut notifications = NotificationUi::new(ui.ctx().clone());
                    for trace_id in ["e2aba56a", "0b3c41f9", "77d0e2c1"] {
                        notifications.add(
                            Notification::new(
                                NotificationLevel::Error,
                                "Failed to load the blueprint, skipping it",
                            )
                            .with_details([
                                "Endpoint: /GetRrdManifest".to_owned(),
                                format!("trace-id: {trace_id}"),
                            ]),
                        );
                    }
                    notifications
                });

                notifications.show_toasts(ui.ctx());
            });

    harness.run();
    harness.snapshot("notification_repeated");

    harness.get_by_label("Details").click();
    harness.run();
    harness.snapshot("notification_repeated_expanded");
}

/// Details taller than the viewport scroll, and the close button takes down a toast that open
/// details would otherwise keep up forever.
#[test]
fn test_long_details_scroll_and_close() {
    let mut notifications: Option<NotificationUi> = None;

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(520.0, 300.0))
            .build_ui(move |ui| {
                let notifications = notifications.get_or_insert_with(|| {
                    let mut notifications = NotificationUi::new(ui.ctx().clone());
                    notifications.add(
                        Notification::new(NotificationLevel::Error, "Failed to load")
                            .with_details((0..40).map(|i| format!("line {i}"))),
                    );
                    notifications
                });

                notifications.show_toasts(ui.ctx());
            });

    harness.run();
    harness.get_by_label("Details").click();
    harness.run();
    harness.snapshot("notification_long_details_expanded");

    harness.get_by_label("Close").click();
    harness.run();
    assert!(
        harness.query_by_label("Details").is_none(),
        "the toast should be gone"
    );
}
