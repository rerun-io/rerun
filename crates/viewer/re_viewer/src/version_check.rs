use std::time::Duration;

use ehttp::{Request, Response};
use re_build_info::CrateVersion;

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
/// A newer release is announced with [`re_log::info!`], while request and parsing failures are only logged at debug level.
///
/// No request is made when `app_env` is [`AppEnvironment::Test`], so tests never produce update notifications or depend on network access.
/// Passing the fetch function in makes the complete request and response flow testable without contacting GitHub.
pub fn check_for_new_version(
    current_version: CrateVersion<'static>,
    app_env: &AppEnvironment,
    fetch: impl FnOnce(Request, OnResponse),
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
            handle_response(current_version, download_url, response);
        }),
    );
}

fn handle_response(
    current_version: CrateVersion<'static>,
    download_url: &'static str,
    response: Result<Response, String>,
) {
    let response = match response {
        Ok(response) if response.ok => response,
        Ok(response) => {
            re_log::debug!(
                "Failed to check for a new Rerun version: {} {}",
                response.status,
                response.status_text
            );
            return;
        }
        Err(err) => {
            re_log::debug!("Failed to check for a new Rerun version: {err}");
            return;
        }
    };

    let release = match serde_json::from_slice::<LatestRelease>(&response.bytes) {
        Ok(release) => release,
        Err(err) => {
            re_log::debug!("Failed to parse the latest Rerun release: {err}");
            return;
        }
    };

    let latest_version = match CrateVersion::try_parse(&release.tag_name) {
        Ok(version) => version,
        Err(err) => {
            re_log::debug!(
                "Failed to parse the latest Rerun version {:?}: {err}",
                release.tag_name
            );
            return;
        }
    };

    if current_version < latest_version {
        re_log::info!(
            "A newer version of Rerun is available: {latest_version} (you are running {current_version}). Download it at {}",
            download_url
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_for_new_version_does_not_fetch_while_testing() {
        let mut fetched = false;
        check_for_new_version(
            CrateVersion::new(0, 35, 0),
            &AppEnvironment::Test,
            |_, _| fetched = true,
        );
        assert!(!fetched);
    }

    #[test]
    fn check_for_new_version_fetches_and_logs_an_update() {
        re_log::setup_logging();

        for (app_env, expected_url) in [
            (AppEnvironment::Custom("test".to_owned()), INSTALL_RERUN_URL),
            (
                AppEnvironment::PythonSdk(re_log_types::PythonVersion {
                    major: 3,
                    minor: 13,
                    patch: 0,
                    suffix: String::new(),
                }),
                PYTHON_SDK_URL,
            ),
        ] {
            let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::INFO);

            // Mock http request so we don't actually depend on changing externalities.
            let response_body = serde_json::json!({ "tag_name": "0.36.3" });

            let mut fetched = false;
            check_for_new_version(
                CrateVersion::new(0, 35, 0),
                &app_env,
                |request, on_response| {
                    fetched = true;
                    assert_eq!(request.url, LATEST_RELEASE_URL);
                    on_response(Ok(Response {
                        url: request.url,
                        ok: true,
                        status: 200,
                        status_text: "OK".to_owned(),
                        headers: ehttp::Headers::default(),
                        bytes: response_body.to_string().into_bytes(),
                    }));
                },
            );

            assert!(fetched);
            assert!(log_rx.try_iter().any(|message| {
                message.message
                    == format!(
                        "A newer version of Rerun is available: 0.36.3 (you are running 0.35.0). Download it at {expected_url}"
                    )
            }));
        }
    }
}
