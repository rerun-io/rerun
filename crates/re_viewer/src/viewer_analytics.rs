//! Most analytics events collected by the Rerun Viewer are defined in this file.
//!
//! All events are defined in the `re_analytics` crate: <https://github.com/rerun-io/rerun/blob/main/crates/re_analytics/src/event.rs>
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.

mod event;

use re_analytics::Analytics;
use re_analytics::AnalyticsError;

use crate::AppEnvironment;

pub struct ViewerAnalytics {
    app_env: AppEnvironment,
    analytics: Analytics,
}

impl ViewerAnalytics {
    #[allow(unused_mut, clippy::let_and_return)]
    pub fn new(app_env: AppEnvironment) -> Result<Self, AnalyticsError> {
        re_tracing::profile_function!();

        let analytics = Analytics::new(std::time::Duration::from_secs(2))?;

        Ok(Self { app_env, analytics })
    }

    /// When the viewer is first started
    pub fn on_viewer_started(&self, build_info: re_build_info::BuildInfo) {
        re_tracing::profile_function!();

        self.analytics.record(event::identify(
            self.analytics.config(),
            build_info,
            &self.app_env,
        ));
        self.analytics.record(event::viewer_started(&self.app_env));
    }

    /// When we have loaded the start of a new recording.
    pub fn on_open_recording(&self, entity_db: &re_entity_db::EntityDb) {
        if entity_db.store_kind() != re_log_types::StoreKind::Recording {
            return;
        }

        self.analytics
            .record(event::open_recording(&self.app_env, entity_db));
    }
}
