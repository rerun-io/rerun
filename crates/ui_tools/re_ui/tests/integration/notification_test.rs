use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::Vec2;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{Harness, HarnessBuilder};
use re_ui::notifications::{Notification, NotificationAction, NotificationLevel, NotificationUi};

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

fn harness_builder<State>() -> HarnessBuilder<State> {
    re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(400.0, 280.0))
}

const PRIVACY_URL: &str = "https://rerun.invalid/privacy";

/// The start of the notice text, before its inline link.
const NOTICE_START: &str = "We collect usage data";

/// A one-time notice with an inline link and two actions, which records which action ran.
#[derive(Default)]
struct OneTimeNotice {
    turn_off_ran: Arc<AtomicBool>,
    settings_ran: Arc<AtomicBool>,
}

impl OneTimeNotice {
    fn notification(&self) -> Notification {
        let turn_off_ran = Arc::clone(&self.turn_off_ran);
        let settings_ran = Arc::clone(&self.settings_ran);
        Notification::new(
            NotificationLevel::Info,
            format!("{NOTICE_START} to improve our products. [Privacy policy]({PRIVACY_URL})."),
        )
        .with_action(NotificationAction::new("Turn off", move || {
            turn_off_ran.store(true, Ordering::Relaxed);
        }))
        .with_action(NotificationAction::new("Settings", move || {
            settings_ran.store(true, Ordering::Relaxed);
        }))
        .show_once(egui::Id::unique("one_time_notice"))
    }

    fn turn_off_ran(&self) -> bool {
        self.turn_off_ran.load(Ordering::Relaxed)
    }

    fn settings_ran(&self) -> bool {
        self.settings_ran.load(Ordering::Relaxed)
    }
}

/// Whether the one-time notice would show again on the next start, which reuses the persisted
/// egui memory.
fn shows_again<State>(harness: &Harness<'_, State>) -> bool {
    let mut next_start = NotificationUi::new(harness.ctx.clone());
    next_start.add(OneTimeNotice::default().notification());
    !next_start.notifications().is_empty()
}

/// Shows `notification` as a toast, next to the button that opens the notification panel.
fn notification_harness(notification: Notification) -> Harness<'static> {
    let mut notification = Some(notification);
    let mut notifications: Option<NotificationUi> = None;

    harness_builder().build_ui(move |ui| {
        let notifications = notifications.get_or_insert_with(|| {
            let mut notifications = NotificationUi::new(ui.ctx().clone());
            if let Some(notification) = notification.take() {
                notifications.add(notification);
            }
            notifications
        });

        // A bar like the viewer's top panel, since the notification panel opens below it.
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 24.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| notifications.notification_toggle_button(ui),
        );
        notifications.show_toasts(ui.ctx());
    })
}

fn open_panel(harness: &mut Harness<'_>) {
    harness.get_by_label_contains("Notification toggle").click();
    harness.run();
}

#[test]
fn test_one_time_notice_toast() {
    let mut harness = notification_harness(OneTimeNotice::default().notification());
    harness.run();
    harness.snapshot("notification_one_time_notice_toast");
}

/// In the panel, a one-time notice offers "Dismiss" instead of "Don't show again".
#[test]
fn test_one_time_notice_panel() {
    let mut harness = notification_harness(OneTimeNotice::default().notification());
    harness.run();
    open_panel(&mut harness);
    harness.snapshot("notification_one_time_notice_panel");
}

/// On a toast with actions, only the action buttons and links respond.
/// An action closes the toast and leaves the notification in the panel, where an action removes it.
#[test]
fn test_notification_actions() {
    let notice = OneTimeNotice::default();
    let mut harness = notification_harness(notice.notification());
    harness.run();

    // The message text is selectable and takes the click, so click the toast's padding next to it.
    let text_rect = harness.get_by_label_contains(NOTICE_START).rect();
    let pos = egui::pos2(text_rect.left() - 2.0, text_rect.center().y);
    harness.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.step();
    assert_eq!(
        harness.output().platform_output.commands,
        [],
        "a click on the body should neither open a link nor copy the text"
    );

    harness.get_by_label("Privacy policy").click();
    harness.step();
    assert_eq!(
        harness.output().platform_output.commands,
        [egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab(
            PRIVACY_URL
        ))],
        "the inline link should open in a new tab"
    );

    harness.get_by_label("Turn off").click();
    harness.run();
    assert!(
        notice.turn_off_ran() && !notice.settings_ran(),
        "only the clicked action should run"
    );
    assert!(
        harness.query_by_label_contains(NOTICE_START).is_none(),
        "the toast should be closed"
    );

    open_panel(&mut harness);
    harness.get_by_label("Settings").click();
    harness.run();
    assert!(notice.settings_ran(), "the action in the panel should run");
    harness.get_by_label("No notifications yet.");
}

/// A one-time notice counts as read once its toast timed out, but the toast's time only starts on
/// the frame after it first appears, since that frame may follow a long idle period.
#[test]
fn test_one_time_notice_remembered_on_timeout_not_on_first_frame() {
    // Each frame is longer than a toast lives, like the first frame after the viewer was idle.
    let mut harness = harness_builder().with_step_dt(5.0).build_ui_state(
        |ui, notifications: &mut Option<NotificationUi>| {
            notifications
                .get_or_insert_with(|| NotificationUi::new(ui.ctx().clone()))
                .show_toasts(ui.ctx());
        },
        None,
    );
    harness
        .state_mut()
        .as_mut()
        .expect("building the harness runs a frame, which creates the notification ui")
        .add(OneTimeNotice::default().notification());

    harness.step();
    assert!(
        shows_again(&harness),
        "the first frame must not use up the toast's time"
    );

    harness.step();
    harness.step();
    assert!(
        harness.query_by_label_contains(NOTICE_START).is_none(),
        "the toast should have timed out"
    );
    assert!(
        !shows_again(&harness),
        "a toast that timed out should remember the notice"
    );
}

/// Closing the toast counts as having read a one-time notice.
#[test]
fn test_one_time_notice_remembered_on_toast_close() {
    let mut harness = notification_harness(OneTimeNotice::default().notification());
    harness.run();

    harness.get_by_label("Close").click();
    harness.run();

    assert!(
        !shows_again(&harness),
        "closing the toast should remember the notice"
    );
}
