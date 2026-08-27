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
                re_ui::apply_style_and_install_loaders(ui.ctx());

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
                re_ui::apply_style_and_install_loaders(ui.ctx());

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
    let err =
        re_redap_client::ApiError::tonic(&re_uri::Origin::test(), status, "/GetTableSchema failed");

    // `target: "re_ui"` so it passes the notification relevance filter (rerun-crate + WARN).
    re_log::error!(target: "re_ui", "An error occurred: {err}");

    let log_msg = log_rx
        .try_recv()
        .expect("the channel logger should have captured the error");

    let mut notifications: Option<NotificationUi> = None;

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(520.0, 280.0))
            .build_ui(move |ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

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
