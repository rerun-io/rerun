use egui::Vec2;
use re_ui::notifications::NotificationUi;

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
}

/// A field value containing [`re_error::DETAILS_SEPARATOR`] should have the part after the
/// separator moved into the collapsible details section instead of being shown inline.
///
/// This is the shape produced by `#[tracing::instrument(err)]`: an event with no message and
/// the whole error — details and all — in an `error` field.
#[test]
fn test_notification_with_details_in_field() {
    let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::INFO);
    re_log::setup_logging();

    let error_text = re_error::format_with_details(
        "/GetTableSchema failed (Internal) (trace-id: ad66019921fce81f3f56462f9a8dbd63), invalid lance input",
        "metadata: {\"content-length\": \"0\", \"date\": \"Wed, 08 Jul 2026 15:28:04 GMT\"}",
    );
    re_log::error!(target: "re_ui", error = %error_text);

    let log_msg = log_rx
        .try_recv()
        .expect("the channel logger should have captured the error");

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
    harness.snapshot("notification_with_details_in_field");
}
