// Header consts, the `RerunVersionInterceptor`, the tower layer helpers, and
// the `PropagateHeaders` middleware live in the `re_grpc_headers` utility
// crate so they can be shared with `crates/utils` callers (which can't depend
// on `crates/store`). Re-exported here so existing `re_protos::headers::*`
// imports keep working without churn.
pub use re_grpc_headers::*;

use crate::EntryName;

/// Extension trait for [`tonic::Request`] to inject Rerun Data Protocol headers into gRPC requests.
///
/// Example:
/// ```
/// # use re_protos::headers::RerunHeadersInjectorExt as _;
/// # use re_log_types::EntryName;
/// let entry_name = EntryName::new("my_entry").unwrap();
/// let mut req = tonic::Request::new(()).with_entry_name(entry_name);
/// ```
pub trait RerunHeadersInjectorExt: Sized {
    fn with_entry_id(self, entry_id: re_log_types::EntryId) -> Self;

    fn with_entry_name(self, entry_name: EntryName) -> Self;

    fn with_metadata(self, md: &tonic::metadata::MetadataMap) -> Self;
}

impl<T> RerunHeadersInjectorExt for tonic::Request<T> {
    fn with_entry_id(mut self, entry_id: re_log_types::EntryId) -> Self {
        let value: tonic::metadata::AsciiMetadataValue = entry_id
            .to_string()
            .parse()
            .expect("EntryId Display always yields valid ASCII metadata");
        self.metadata_mut()
            .insert(RERUN_HTTP_HEADER_ENTRY_ID, value);
        self
    }

    fn with_entry_name(mut self, entry_name: EntryName) -> Self {
        let value =
            tonic::metadata::BinaryMetadataValue::from_bytes(entry_name.as_str().as_bytes());
        self.metadata_mut()
            .insert_bin(RERUN_HTTP_HEADER_ENTRY_NAME, value);
        self
    }

    fn with_metadata(mut self, md: &tonic::metadata::MetadataMap) -> Self {
        if let Some(entry_id) = md.get(RERUN_HTTP_HEADER_ENTRY_ID).cloned() {
            self.metadata_mut()
                .insert(RERUN_HTTP_HEADER_ENTRY_ID, entry_id);
        }

        if let Some(entry_name) = md.get_bin(RERUN_HTTP_HEADER_ENTRY_NAME).cloned() {
            self.metadata_mut()
                .insert_bin(RERUN_HTTP_HEADER_ENTRY_NAME, entry_name);
        }

        if let Some(auth) = md.get(HTTP_HEADER_AUTHORIZATION).cloned() {
            self.metadata_mut().insert(HTTP_HEADER_AUTHORIZATION, auth);
        }

        self
    }
}

/// Extension trait for [`tonic::Request`] to extract Rerun Data Protocol headers from gRPC requests.
///
/// Example:
/// ```
/// # use re_protos::headers::RerunHeadersExtractorExt as _;
/// # let req = tonic::Request::new(());
/// let entry_id = req.entry_id().unwrap();
/// ```
pub trait RerunHeadersExtractorExt {
    fn entry_id(&self) -> tonic::Result<Option<re_log_types::EntryId>>;

    fn entry_name(&self) -> tonic::Result<Option<EntryName>>;
}

impl<T> RerunHeadersExtractorExt for tonic::Request<T> {
    fn entry_id(&self) -> tonic::Result<Option<re_log_types::EntryId>> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_ID;

        let Some(entry_id) = self.metadata().get(HEADER) else {
            return Ok(None);
        };

        let entry_id = entry_id.to_str().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;
        let entry_id = entry_id.parse().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;

        Ok(Some(entry_id))
    }

    fn entry_name(&self) -> tonic::Result<Option<EntryName>> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_NAME;

        let Some(entry_name) = self.metadata().get_bin(HEADER) else {
            return Ok(None);
        };

        let entry_name = entry_name.to_bytes().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_name:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;
        let entry_name = String::from_utf8(entry_name.to_vec()).map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_name:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;
        let entry_name = EntryName::new(&entry_name)
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;

        Ok(Some(entry_name))
    }
}

/// Resolves the entry id of an entry-addressed request.
///
/// The id is read from the `x-rerun-entry-id` HTTP header first; if absent, the caller-supplied
/// deprecated body id is used as a fallback, to stay compatible with old clients.
/// If both the header and the body carry an id, they must agree: a mismatch is rejected with
/// [`tonic::Code::InvalidArgument`].
///
/// The body fallback can be removed once we can stop being backwards compatible with
/// clients older or equal to 0.32.x.
pub fn resolve_entry_id<T>(
    req: &tonic::Request<T>,
    body_id: Option<&crate::common::v1alpha1::EntryId>,
) -> tonic::Result<re_log_types::EntryId> {
    let header_id = req.entry_id()?;
    let body_id: Option<re_log_types::EntryId> = body_id.map(|id| (*id).try_into()).transpose()?;

    match (header_id, body_id) {
        (Some(header_id), Some(body_id)) if header_id != body_id => {
            Err(tonic::Status::invalid_argument(format!(
                "mismatched entry ids: the '{RERUN_HTTP_HEADER_ENTRY_ID}' header says '{header_id}', but the request body says '{body_id}'"
            )))
        }
        (Some(id), _) | (None, Some(id)) => Ok(id),
        (None, None) => Err(tonic::Status::invalid_argument(
            "missing entry id in request header and body",
        )),
    }
}
