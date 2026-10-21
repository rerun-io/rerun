use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::LazyLock;

use re_log_types::EntryId;

use re_protos::cloud::v1alpha1;
use re_protos::headers::{RerunHeadersExtractorExt as _, RerunHeadersInjectorExt as _};

/// Tracks the dataset revision watermark for all datasets shared by all clients and response streams.
///
/// Revisions start at one. The server reports zero when a dataset has no promoted revision, and
/// ignores a zero watermark on requests, so zero is never stored here.
pub struct DatasetRevisions(re_mutex::RwLock<HashMap<EntryId, NonZeroU64>>);

/// Returns the global dataset revisions tracker.
pub fn dataset_revisions() -> &'static DatasetRevisions {
    static REVISIONS: LazyLock<DatasetRevisions> =
        LazyLock::new(|| DatasetRevisions(re_mutex::RwLock::new(HashMap::new())));
    &REVISIONS
}

impl DatasetRevisions {
    /// Returns the current watermark for the given entry id, if known.
    pub fn get(&self, entry_id: EntryId) -> Option<NonZeroU64> {
        self.0.read().get(&entry_id).copied()
    }

    /// Update the watermark for the given entry id.
    ///
    /// A zero revision means the server has no promoted revision and is ignored.
    pub fn observe(&self, entry_id: EntryId, revision: u64) {
        let Some(revision) = NonZeroU64::new(revision) else {
            return;
        };
        let mut revisions = self.0.write();
        let watermark = revisions.entry(entry_id).or_insert(revision);
        *watermark = (*watermark).max(revision);
    }

    /// Update the watermark for the given dataset response metadata, if valid.
    pub fn observe_meta(&self, meta: Option<&v1alpha1::DatasetResponseMeta>) {
        let Some(meta) = meta else {
            return;
        };
        let Some(entry_id) = meta.entry_id.and_then(|id| id.try_into().ok()) else {
            return;
        };
        self.observe(entry_id, meta.dataset_revision);
    }

    /// Attach the current watermark to the given gRPC request, if available.
    pub fn stamp<T>(&self, request: tonic::Request<T>) -> tonic::Request<T> {
        let Ok(Some(entry_id)) = request.entry_id() else {
            return request;
        };
        if let Some(revision) = self.get(entry_id) {
            request.with_dataset_revision(revision.get())
        } else {
            request
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: EntryId, revision: u64) -> v1alpha1::DatasetResponseMeta {
        v1alpha1::DatasetResponseMeta {
            entry_id: Some(id.into()),
            dataset_revision: revision,
        }
    }

    #[test]
    fn monotonic_watermarks_and_request_headers() {
        let revisions = dataset_revisions();
        let id = EntryId::new();
        let other = EntryId::new();
        revisions.observe_meta(Some(&meta(other, 9)));
        for (revision, expected) in [
            (None, None),
            (Some(0), None),
            (Some(7), Some(7)),
            (Some(0), Some(7)),
            (Some(3), Some(7)),
            (Some(10), Some(10)),
        ] {
            revisions.observe_meta(revision.map(|revision| meta(id, revision)).as_ref());
            let request = revisions.stamp(tonic::Request::new(()).with_entry_id(id));
            assert_eq!(request.dataset_revision().unwrap(), expected);
        }
        assert_eq!(revisions.get(other), NonZeroU64::new(9));
    }

    #[test]
    fn requests_without_entry_id_are_not_stamped() {
        for request in [
            tonic::Request::new(()),
            tonic::Request::new(())
                .with_entry_name(re_log_types::EntryName::new("dataset").unwrap()),
        ] {
            let request = dataset_revisions().stamp(request);
            assert_eq!(request.dataset_revision().unwrap(), None);
        }
    }
}
