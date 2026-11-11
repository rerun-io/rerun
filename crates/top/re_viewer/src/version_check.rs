use std::time::Duration;

use ehttp::{Request, Response};
use re_build_info::CrateVersion;
use re_ui::notifications::{Notification, NotificationLevel};

use crate::AppEnvironment;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/rerun-io/rerun/releases/latest";
const INSTALL_RERUN_URL: &str = "https://rerun.io/docs/getting-started/install-rerun";
const PYTHON_SDK_URL: &str = "https://pypi.org/project/rerun-sdk/";

#[derive(serde::Deserialize)]
struct LatestRelease {
    tag_name: String,
}

type OnResponse = Box<dyn FnOnce(Result<Response, String>) + Send>;

/// Checks GitHub for a newer stable Rerun release without blocking viewer startup.
///
/// The supplied `fetch` function starts an asynchronous request to GitHub's latest-release API.
/// Once the request completes, the response callback parses the release tag as a final [`CrateVersion`] and compares it with `current_version`.
/// A newer release is passed to `notify` as a [`Notification`] that is shown once per release:
/// a user who stays on an old version is told about each new release once, not on every start.
/// Request and parsing failures are only logged at debug level.
///
/// `fetch` may call its response callback on another thread, so `notify` must not assume it runs on the UI thread.
///
/// No request is made when `app_env` is [`AppEnvironment::Test`], so tests never produce update notifications or depend on network access.
/// Passing the fetch function in makes the complete request and response flow testable without contacting GitHub.
pub fn check_for_new_version(
    current_version: CrateVersion<'static>,
    app_env: &AppEnvironment,
    fetch: impl FnOnce(Request, OnResponse),
    notify: impl FnOnce(Notification) + Send + 'static,
) {
    if app_env.is_test() {
        return;
    }

    let request = Request::get(LATEST_RELEASE_URL)
        .with_header("Accept", "application/vnd.github+json")
        .with_header("X-GitHub-Api-Version", "2022-11-28")
        .with_timeout(Some(Duration::from_secs(5)));

    let download_url = match app_env {
        AppEnvironment::PythonSdk(_) => PYTHON_SDK_URL,
        _ => INSTALL_RERUN_URL,
    };

    fetch(
        request,
        Box::new(move |response| {
            if let Some(notification) = handle_response(current_version, download_url, response) {
                notify(notification);
            }
        }),
    );
}

fn handle_response(
    current_version: CrateVersion<'static>,
    download_url: &'static str,
    response: Result<Response, String>,
) -> Option<Notification> {
    let response = match response {
        Ok(response) if response.ok => response,
        Ok(response) => {
            re_log::debug!(
                "Failed to check for a new Rerun version: {} {}",
                response.status,
                response.status_text
            );
            return None;
        }
        Err(err) => {
            re_log::debug!("Failed to check for a new Rerun version: {err}");
            return None;
        }
    };

    let release = match serde_json::from_slice::<LatestRelease>(&response.bytes) {
        Ok(release) => release,
        Err(err) => {
            re_log::debug!("Failed to parse the latest Rerun release: {err}");
            return None;
        }
    };

    let latest_version = match CrateVersion::try_parse(&release.tag_name) {
        Ok(version) => version,
        Err(err) => {
            re_log::debug!(
                "Failed to parse the latest Rerun version {:?}: {err}",
                release.tag_name
            );
            return None;
        }
    };

    (current_version < latest_version).then(|| {
        let text = format!(
            "Rerun {latest_version} is available (you are running {current_version}). [Download…]({download_url})"
        );
        re_log::debug!("{text}");
        Notification::new(NotificationLevel::Info, text)
            .show_once(egui::Id::unique(("new_version_notice", latest_version)))
    })
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable as _;
    use re_ui::notifications::NotificationUi;

    use super::*;

    /// A successful response from the latest-release API.
    fn release_response(url: String, tag: &str) -> Response {
        Response {
            url,
            ok: true,
            status: 200,
            status_text: "OK".to_owned(),
            headers: ehttp::Headers::default(),
            bytes: serde_json::json!({ "tag_name": tag })
                .to_string()
                .into_bytes(),
        }
    }

    fn new_version_notice(tag: &str) -> Notification {
        let response = release_response(LATEST_RELEASE_URL.to_owned(), tag);
        handle_response(CrateVersion::new(0, 35, 0), INSTALL_RERUN_URL, Ok(response))
            .expect("a newer release should produce a notification")
    }

    #[test]
    fn check_for_new_version_does_not_fetch_while_testing() {
        let mut fetched = false;
        check_for_new_version(
            CrateVersion::new(0, 35, 0),
            &AppEnvironment::Test,
            |_, _| fetched = true,
            |_| {},
        );
        assert!(!fetched);
    }

    #[test]
    fn check_for_new_version_fetches_and_notifies_an_update() {
        for (app_env, expected_url) in [
            (AppEnvironment::Custom("test".to_owned()), INSTALL_RERUN_URL),
            (
                AppEnvironment::PythonSdk(re_log_msg::PythonVersion {
                    major: 3,
                    minor: 13,
                    patch: 0,
                    suffix: String::new(),
                }),
                PYTHON_SDK_URL,
            ),
        ] {
            let mut fetched = false;
            let notified = std::sync::Arc::new(parking_lot::Mutex::new(None));
            check_for_new_version(
                CrateVersion::new(0, 35, 0),
                &app_env,
                |request, on_response| {
                    fetched = true;
                    assert_eq!(request.url, LATEST_RELEASE_URL);
                    on_response(Ok(release_response(request.url, "0.36.3")));
                },
                {
                    let notified = notified.clone();
                    move |notification| *notified.lock() = Some(notification)
                },
            );

            assert!(fetched);
            let notification = notified.lock().take().unwrap();
            assert_eq!(
                notification.text(),
                format!(
                    "Rerun 0.36.3 is available (you are running 0.35.0). [Download…]({expected_url})"
                )
            );
        }
    }

    /// Once the user has seen the notice for a release, it does not show again for that release,
    /// but does for the next one: the show-once key must include the release version.
    #[test]
    fn new_version_notice_shows_once_per_release() {
        let mut notifications = None;
        let mut harness =
            re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, [400.0, 280.0]).build_ui(
                |ui| {
                    notifications
                        .get_or_insert_with(|| {
                            let mut notifications = NotificationUi::new(ui.ctx().clone());
                            notifications.add(new_version_notice("0.36.3"));
                            notifications
                        })
                        .notification_toggle_button(ui);
                },
            );

        // Opening the notification panel counts as having seen the notice.
        harness.run();
        harness.get_by_label_contains("Notification toggle").click();
        harness.run();

        // A fresh `NotificationUi` on the same egui memory, like the next start of the viewer.
        let shows_on_next_start = |tag| {
            let mut notifications = NotificationUi::new(harness.ctx.clone());
            notifications.add(new_version_notice(tag));
            !notifications.notifications().is_empty()
        };
        assert!(
            !shows_on_next_start("0.36.3"),
            "the notice for an already seen release should not show again"
        );
        assert!(
            shows_on_next_start("0.37.0"),
            "the notice for a newer release should show"
        );
    }
}
