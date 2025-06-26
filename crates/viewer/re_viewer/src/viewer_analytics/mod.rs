//! Most analytics events collected by the Rerun Viewer are defined in this file.
//!
//! All events are defined in the `re_analytics` crate: <https://github.com/rerun-io/rerun/blob/main/crates/utils/re_analytics/src/event.rs>
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.

#[cfg(feature = "analytics")]
mod event;

#[cfg(feature = "analytics")]
mod wsl;

use crate::AppEnvironment;
use crate::StartupOptions;

pub struct ViewerAnalytics {
    #[cfg(feature = "analytics")]
    app_env: AppEnvironment,

    // NOTE: Optional because it is possible to have the `analytics` feature flag enabled
    // while at the same time opting-out of analytics at run-time.
    #[cfg(feature = "analytics")]
    analytics: Option<&'static re_analytics::Analytics>,
}

impl ViewerAnalytics {
    #[allow(unused_mut, clippy::let_and_return)]
    pub fn new(startup_options: &StartupOptions, app_env: AppEnvironment) -> Self {
        re_tracing::profile_function!();

        #[cfg(feature = "analytics")]
        {
            let analytics = if startup_options.is_in_notebook {
                None
            } else {
                re_analytics::Analytics::global_or_init()
            };
            Self { app_env, analytics }
        }

        #[cfg(not(feature = "analytics"))]
        {
            let _ = (startup_options, app_env);
            Self {}
        }
    }

    /// When the viewer is first started
    #[allow(unused_variables)]
    pub fn on_viewer_started(
        &self,
        build_info: re_build_info::BuildInfo,
        adapter_backend: wgpu::Backend,
        device_tier: re_renderer::device_caps::DeviceCapabilityTier,
    ) {
        re_tracing::profile_function!();

        #[cfg(feature = "analytics")]
        {
            let Some(analytics) = self.analytics.as_ref() else {
                return;
            };

            analytics.record(event::identify(
                analytics.config(),
                build_info,
                &self.app_env,
            ));
            analytics.record(event::viewer_started(
                &self.app_env,
                adapter_backend,
                device_tier,
            ));
        }
    }

    /// When we have loaded the start of a new recording.
    pub fn on_open_recording(&self, entity_db: &re_entity_db::EntityDb) {
        #[cfg(feature = "analytics")]
        {
            if entity_db.store_kind() != re_log_types::StoreKind::Recording {
                return;
            }

            let Some(analytics) = self.analytics.as_ref() else {
                return;
            };

            if let Some(event) = event::open_recording(&self.app_env, entity_db) {
                analytics.record(event);
            }
        }

        #[cfg(not(feature = "analytics"))]
        let _ = entity_db;
    }
}
