#![allow(clippy::result_large_err)] // we're just returning tonic::Status

/// The HTTP header key to pass an entry ID to the `RerunCloudService` APIs.
pub const RERUN_HTTP_HEADER_ENTRY_ID: &str = "x-rerun-entry-id";

/// The HTTP header key to pass an entry name to the `RerunCloudService` APIs.
///
/// This will automatically be resolved to an entry ID, as long as a dataset with the associated
/// name can be found in the database.
///
/// This is serialized as base64-encoded data (hence `-bin`), since entry names can be any UTF8 strings,
/// while HTTP2 headers only support ASCII.
pub const RERUN_HTTP_HEADER_ENTRY_NAME: &str = "x-rerun-entry-name-bin";

/// Helper struct to inject Rerun Data Protocol headers into gRPC requests.
///
/// Example:
/// ```
/// # use re_protos::headers::RerunHeadersInjector;
/// let mut req = tonic::Request::new(());
/// RerunHeadersInjector(&mut req).with_entry_name("droid:sample2k").unwrap();
/// ```
pub struct RerunHeadersInjector<'a, T>(pub &'a mut tonic::Request<T>);

impl<T> RerunHeadersInjector<'_, T> {
    pub fn with_entry_id(&mut self, entry_id: impl AsRef<str>) -> Result<(), tonic::Status> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_ID;

        let entry_id = entry_id.as_ref();
        let entry_id = entry_id.parse().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;

        self.0.metadata_mut().insert(HEADER, entry_id);

        Ok(())
    }

    #[expect(clippy::unnecessary_wraps)] // for consistency / future-proofing
    pub fn with_entry_name(&mut self, entry_name: impl AsRef<str>) -> Result<(), tonic::Status> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_NAME;

        let entry_name = entry_name.as_ref();
        let entry_name = tonic::metadata::BinaryMetadataValue::from_bytes(entry_name.as_bytes());

        self.0.metadata_mut().insert_bin(HEADER, entry_name);

        Ok(())
    }

    pub fn with_metadata(&mut self, md: &tonic::metadata::MetadataMap) {
        if let Some(entry_id) = md.get(RERUN_HTTP_HEADER_ENTRY_ID).cloned() {
            self.0
                .metadata_mut()
                .insert(RERUN_HTTP_HEADER_ENTRY_ID, entry_id);
        }

        if let Some(entry_name) = md.get_bin(RERUN_HTTP_HEADER_ENTRY_NAME).cloned() {
            self.0
                .metadata_mut()
                .insert_bin(RERUN_HTTP_HEADER_ENTRY_NAME, entry_name);
        }
    }
}

/// Helper struct to extract Rerun Data Protocol headers from gRPC requests.
///
/// Example:
/// ```
/// # use re_protos::headers::RerunHeadersExtractor;
/// # let req = tonic::Request::new(());
/// let entry_id = RerunHeadersExtractor(&req).entry_id().unwrap();
/// ```
pub struct RerunHeadersExtractor<'a, T>(pub &'a tonic::Request<T>);

impl<'a, T> RerunHeadersExtractor<'a, T> {
    pub fn entry_id(&self) -> Result<Option<&'a str>, tonic::Status> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_ID;

        let Some(entry_id) = self.0.metadata().get(HEADER) else {
            return Ok(None);
        };

        let entry_id = entry_id.to_str().map_err(|err| {
            tonic::Status::invalid_argument(format!(
                "'{entry_id:?}' is not a valid value for '{HEADER}': {err:#}"
            ))
        })?;

        Ok(Some(entry_id))
    }

    pub fn entry_name(&self) -> Result<Option<String>, tonic::Status> {
        const HEADER: &str = RERUN_HTTP_HEADER_ENTRY_NAME;

        let Some(entry_name) = self.0.metadata().get_bin(HEADER) else {
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

        Ok(Some(entry_name))
    }
}
