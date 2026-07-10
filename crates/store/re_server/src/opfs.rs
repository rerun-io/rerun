use percent_encoding::percent_decode_str;
use url::Url;

const OPFS_URL_PREFIX: &str = "opfs:///";

#[cfg(target_arch = "wasm32")]
pub use self::browser::{PathBuf, metadata, read};

pub(crate) fn parse_url(raw_url: &str) -> tonic::Result<Vec<String>> {
    // TODO(grtlr): Add some point we want to bring some of the
    // strictness to regular files on native too.

    let parsed = Url::parse(raw_url)
        .map_err(|err| invalid_url(raw_url, format_args!("could not parse URL: {err}")))?;

    if parsed.scheme() != "opfs" {
        return Err(invalid_url(raw_url, "scheme must be opfs"));
    }

    if parsed.host_str().is_some() {
        return Err(invalid_url(raw_url, "hosts are not supported"));
    }

    if parsed.query().is_some() {
        return Err(invalid_url(raw_url, "query strings are not supported"));
    }

    if parsed.fragment().is_some() {
        return Err(invalid_url(raw_url, "fragments are not supported"));
    }

    // Require the canonical empty-authority form `opfs:///…`. `Url::parse` lower-cases the
    // scheme, so a prefix check on the serialized URL enforces that form and rejects `opfs:/…`,
    // which has no authority.
    if !parsed.as_str().starts_with(OPFS_URL_PREFIX) {
        return Err(invalid_url(raw_url, "URL must start with opfs:///"));
    }

    // Segments come from the parsed path, not the raw string: `Url` has already resolved any
    // `.`/`..` segments relative to the root (they cannot escape the OPFS sandbox and so never
    // reach this point), so the segments validated here are exactly the ones we open.
    let raw_path = parsed.path().strip_prefix('/').unwrap_or_default();
    if raw_path.is_empty() {
        return Err(invalid_url(raw_url, "path must not be empty"));
    }

    let mut components = Vec::new();
    for raw_component in raw_path.split('/') {
        if raw_component.is_empty() {
            return Err(invalid_url(raw_url, "path segments must not be empty"));
        }

        let component = percent_decode_str(raw_component)
            .decode_utf8()
            .map_err(|err| {
                invalid_url(
                    raw_url,
                    format_args!("path segment is not valid UTF-8: {err}"),
                )
            })?;

        if component.contains('/') {
            return Err(invalid_url(
                raw_url,
                "path segments must not contain encoded / separators",
            ));
        }

        components.push(component.into_owned());
    }

    Ok(components)
}

fn invalid_url(raw_url: &str, reason: impl std::fmt::Display) -> tonic::Status {
    const EXPECTED_OPFS_URL: &str = "opfs:///path/to/file.rrd";
    tonic::Status::invalid_argument(format!(
        "invalid OPFS URL, expected {EXPECTED_OPFS_URL}: {reason}\nURL: {raw_url}"
    ))
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::io;

    use url::Url;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{DomException, FileSystemDirectoryHandle, FileSystemFileHandle};

    use super::parse_url;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PathBuf {
        components: Vec<String>,
    }

    impl PathBuf {
        pub fn from_url(url: &Url) -> tonic::Result<Self> {
            Ok(Self {
                components: parse_url(url.as_str())?,
            })
        }
    }

    pub struct Metadata {
        len: u64,
        is_file: bool,
    }

    impl Metadata {
        pub fn is_file(&self) -> bool {
            self.is_file
        }

        pub fn len(&self) -> u64 {
            self.len
        }
    }

    pub async fn metadata(path: &PathBuf) -> io::Result<Metadata> {
        let path = path.clone();
        run_local(async move {
            match open_file(&path).await {
                Ok(file_handle) => {
                    let file: web_sys::File = await_js(file_handle.get_file()).await?;
                    let blob: &web_sys::Blob = file.as_ref();
                    Ok(Metadata {
                        len: blob.size() as u64,
                        is_file: true,
                    })
                }
                Err(err) if err.kind() == io::ErrorKind::InvalidInput => Ok(Metadata {
                    len: 0,
                    is_file: false,
                }),
                Err(err) => Err(err),
            }
        })
        .await
    }

    // TODO(RR-5154): Replace this with something akin to `read_exact_at`, to avoid
    // copying all of the bytes via `to_vec`.
    pub async fn read(path: &PathBuf) -> io::Result<Vec<u8>> {
        let path = path.clone();
        run_local(async move {
            let file_handle = open_file(&path).await?;
            let file: web_sys::File = await_js(file_handle.get_file()).await?;
            let blob: &web_sys::Blob = file.as_ref();
            let buffer: js_sys::ArrayBuffer = await_js(blob.array_buffer()).await?;

            Ok(js_sys::Uint8Array::new(&buffer).to_vec())
        })
        .await
    }

    async fn open_file(path: &PathBuf) -> io::Result<FileSystemFileHandle> {
        let Some((file_name, directory_names)) = path.components.split_last() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "validated OPFS path unexpectedly had no file component",
            ));
        };

        let navigator = web_sys::window()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "OPFS requires a browser Window")
            })?
            .navigator();
        let mut directory: FileSystemDirectoryHandle =
            await_js(navigator.storage().get_directory()).await?;

        for directory_name in directory_names {
            directory = await_js(directory.get_directory_handle(directory_name)).await?;
        }

        await_js(directory.get_file_handle(file_name)).await
    }

    /// Converts a [`js_sys::Promise`] to a Rust `future`.
    async fn await_js<T>(promise: js_sys::Promise) -> io::Result<T>
    where
        T: JsCast,
    {
        JsFuture::from(promise)
            .await
            .map_err(js_to_io_error)?
            .dyn_into()
            .map_err(js_to_io_error)
    }

    /// `tonic` service futures are `Send`, while `JsFuture` is not.
    /// Run browser API work on the local Wasm executor and await only the `Send` oneshot receiver.
    fn run_local<T>(
        future: impl std::future::Future<Output = io::Result<T>> + 'static,
    ) -> impl std::future::Future<Output = io::Result<T>> + Send
    where
        T: Send + 'static,
    {
        let (tx, rx) = futures::channel::oneshot::channel();

        wasm_bindgen_futures::spawn_local(async move {
            let result = future.await;
            tx.send(result).ok();
        });

        async move {
            rx.await.map_err(|_err| {
                io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OPFS browser task was canceled before completion",
                )
            })?
        }
    }

    fn js_to_io_error(value: JsValue) -> io::Error {
        if let Some(exception) = value.dyn_ref::<DomException>() {
            return err_from_dom_exception(exception);
        }

        if let Some(error) = value.dyn_ref::<js_sys::Error>() {
            return err_from_js(error);
        }

        io::Error::other(value.as_string().unwrap_or_else(|| format!("{value:?}")))
    }

    fn err_from_dom_exception(exception: &DomException) -> io::Error {
        let kind = match exception.code() {
            DomException::NOT_FOUND_ERR => io::ErrorKind::NotFound,
            DomException::SECURITY_ERR => io::ErrorKind::PermissionDenied,
            DomException::TYPE_MISMATCH_ERR => io::ErrorKind::InvalidInput,
            _ => io::ErrorKind::Other,
        };

        io::Error::new(kind, exception.message())
    }

    fn err_from_js(error: &js_sys::Error) -> io::Error {
        let name = String::from(error.name());
        let raw_message = String::from(error.message());
        let message = if raw_message.is_empty() {
            name
        } else {
            format!("{name}: {raw_message}")
        };

        io::Error::other(message)
    }
}

#[cfg(test)]
mod tests {
    use tonic::Code;

    use super::parse_url;

    #[test]
    fn parse_url_accepts_opfs_paths() {
        assert_eq!(
            parse_ok("opfs:///uploads/recording.rrd"),
            vec!["uploads", "recording.rrd"]
        );
        assert_eq!(
            parse_ok("OPFS:///uploads/hello%20world.rrd"),
            vec!["uploads", "hello world.rrd"]
        );
        assert_eq!(parse_ok("opfs:///recording.rrd"), vec!["recording.rrd"]);

        // `Url` resolves `.`/`..` relative to the OPFS root before we see the path.
        assert_eq!(
            parse_ok("opfs:///uploads/../recording.rrd"),
            vec!["recording.rrd"]
        );
    }

    #[test]
    fn parse_url_rejects_malformed_urls() {
        for raw_url in [
            "memory:///store/123",
            "opfs:/uploads/recording.rrd",
            "opfs://uploads/recording.rrd",
            "opfs://host/uploads/recording.rrd",
            "opfs:///%FF.rrd",
            "opfs:///",
            "opfs:///uploads//recording.rrd",
            "opfs:///uploads/.",
            "opfs:///uploads/%2e",
            "opfs:///uploads/..",
            "opfs:///uploads/%2e%2e",
            "opfs:///uploads/a%2Fb.rrd",
            "opfs:///uploads/recording.rrd?download=1",
            "opfs:///uploads/recording.rrd#fragment",
        ] {
            assert_invalid(raw_url);
        }
    }

    fn parse_ok(raw_url: &str) -> Vec<String> {
        match parse_url(raw_url) {
            Ok(components) => components,
            Err(err) => panic!("failed to parse {raw_url}: {err}"),
        }
    }

    fn assert_invalid(raw_url: &str) {
        match parse_url(raw_url) {
            Ok(components) => panic!("parsed invalid OPFS URL {raw_url}: {components:?}"),
            Err(err) => assert_eq!(err.code(), Code::InvalidArgument, "{raw_url}"),
        }
    }
}
