use std::time::Duration;

use re_log::{error, trace};

use crate::{Config, Event, PostHogSink};

// TODO(cmc): abstract away the concept of a `Pipeline` behind an actual trait when comes the time
// to support more than just PostHog.

// ---

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

/// An eventual, at-least-once event pipeline, backed by a write-ahead log in local storage.
#[derive(Debug)]
pub struct Pipeline {
    analytics_id: String,
    session_id: String,
}

impl Pipeline {
    pub fn new(
        config: &Config,
        _tick: Duration,
        _sink: PostHogSink,
    ) -> Result<Option<Self>, PipelineError> {
        if !config.analytics_enabled {
            return Ok(None);
        }

        Ok(Some(Self {
            analytics_id: config.analytics_id.clone(),
            session_id: config.session_id.to_string(),
        }))
    }

    // TODO: catchup (can we even list stuff...? `Storage.key()` sounds like our best chance)
    pub fn record(&self, event: Event) {
        let analytics_id = &self.analytics_id;
        let session_id = &self.session_id;

        trace!(
            %analytics_id, %session_id,
            "appending event to current session file..."
        );

        let mut event_str = match serde_json::to_string(&event) {
            Ok(event_str) => event_str,
            Err(err) => {
                error!(%err, %analytics_id, %session_id, "corrupt analytics event: discarding");
                return;
            }
        };
        event_str.push('\n');

        let Some(storage) = local_storage() else {
            // TODO: some error
            return;
        };

        let Ok(session_file) = storage.get_item(&session_id) else {
            // TODO: some error
            return;
        };

        if let Some(mut session_data) = session_file {
            // TODO: ...
            session_data.push_str(&event_str);
            storage.set_item(&session_id, &session_data);
        } else {
            storage.set_item(&session_id, &event_str);
        }
    }

    fn config_key() -> &'static str {
        "rerun_analytics_config"
    }
    fn data_key() -> &'static str {
        "rerun_analytics_data"
    }
}

// ---

// TODO: this is duped in config_web.rs and I'm not happy about it
// TODO: how fast/slow is all of this exactly..?

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn local_storage_get(key: &str) -> Option<String> {
    local_storage().map(|storage| storage.get_item(key).ok())??
}

fn local_storage_set(key: &str, value: &str) {
    local_storage().map(|storage| storage.set_item(key, value));
}
