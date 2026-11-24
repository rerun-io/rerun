//! The latest terminal loading error for each data source.

use ahash::HashMap;
use re_log_channel::LogSource;

/// The latest terminal loading error for each data source.
///
/// Sources are compared ignoring their uri fragments, so an error stays attached to a source
/// however it is opened.
#[derive(Default)]
pub struct LoadingErrors {
    by_source: HashMap<LogSource, String>,
}

impl LoadingErrors {
    /// All errors, keyed by the source that failed.
    pub fn by_source(&self) -> &HashMap<LogSource, String> {
        &self.by_source
    }

    /// The latest error for `source`, if it failed.
    pub fn get(&self, source: &LogSource) -> Option<&str> {
        self.by_source
            .iter()
            .find(|(candidate, _)| candidate.is_same_ignoring_uri_fragments(source))
            .map(|(_, error)| error.as_str())
    }

    /// Records that `source` failed to load.
    pub fn insert(&mut self, source: LogSource, error: String) {
        self.by_source.insert(source, error);
    }

    /// Forgets the error of `source`, e.g. because it is being loaded again.
    pub fn clear(&mut self, source: &LogSource) {
        self.by_source
            .retain(|candidate, _| !candidate.is_same_ignoring_uri_fragments(source));
    }
}

#[cfg(test)]
mod tests {
    use re_log_channel::RecordingOpenBehavior;
    use re_log_types::EntryId;

    use super::*;

    #[test]
    fn loading_error_lookup_ignores_redap_open_behavior() {
        let entry_id = EntryId::new();
        let uri: re_uri::DatasetUri =
            format!("rerun://127.0.0.1:1234/dataset/{entry_id}?segment_id=pid")
                .parse()
                .expect("valid dataset URI");
        let failed_source = LogSource::RedapGrpcStream {
            uri: uri.clone(),
            open_behavior: RecordingOpenBehavior::Open,
        };
        let loading_source = LogSource::RedapGrpcStream {
            uri,
            open_behavior: RecordingOpenBehavior::Background,
        };
        let mut errors = LoadingErrors::default();
        errors.insert(failed_source, "server error".to_owned());

        assert_eq!(errors.get(&loading_source), Some("server error"));
    }
}
