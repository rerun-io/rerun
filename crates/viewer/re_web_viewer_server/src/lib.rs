//! Serves the web viewer wasm/html.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

use std::borrow::Cow;
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const DEFAULT_WEB_VIEWER_SERVER_PORT: u16 = 9090;

// See `Cargo.toml` for docs about the `disable_web_viewer_server` and `trailing_web_viewer` cfgs:
#[cfg(all(not(disable_web_viewer_server), trailing_web_viewer))]
mod trailing_data;

/// Failure to host the web viewer.
#[derive(thiserror::Error, Debug)]
pub enum WebViewerServerError {
    #[error("Could not parse address: {0}")]
    AddrParseFailed(#[from] std::net::AddrParseError),

    #[error("Failed to create server: {source}: ({address})")]
    CreateServerFailed {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
        address: String,
    },

    #[error(transparent)]
    FailedToLoadData(#[from] WebViewerDataError),
}

/// Failure to load the [`WebViewerData`].
#[derive(thiserror::Error, Debug)]
pub enum WebViewerDataError {
    #[error("Failed to get current executable path: {0}")]
    CurrentExe(std::io::Error),

    #[error("Failed to open executable: {source}, path: {path}")]
    OpenFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to read executable metadata: {0}")]
    ExeMetadata(std::io::Error),

    #[error("Failed to open web viewer assets archive: {source}, path: {path}")]
    OpenArchive {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error(
        "This build contains no built-in web viewer assets (RERUN_EXTERNAL_WEB_VIEWER=1). The assets must be loaded from a zip archive on disk, but no archive path was provided."
    )]
    NoBuiltinAssets,

    #[error("Failed to read trailer from executable: {0}")]
    ReadTrailer(std::io::Error),

    #[error(
        "Invalid magic marker in trailing data. Expected {expected:?}, got {actual:?}. This binary was built with RERUN_TRAILING_WEB_VIEWER=1 but the post-processing step (scripts/append_web_viewer.py) has not been completed."
    )]
    InvalidMagic {
        expected: &'static [u8],
        actual: Vec<u8>,
    },

    #[error("Failed to seek to zip offset {offset} in executable: {source}")]
    SeekToZip { offset: u64, source: std::io::Error },

    #[error("Failed to read {size} bytes of zip data: {source}")]
    ReadZip { size: u64, source: std::io::Error },

    #[error("Failed to parse zip archive: {0}. The data may be corrupted.")]
    ParseZip(zip::result::ZipError),

    #[error("Failed to extract file '{name}' from zip archive: {source}")]
    ExtractFile {
        name: String,
        source: zip::result::ZipError,
    },

    #[error("Failed to read file '{name}' contents: {source}")]
    ReadFileContents {
        name: String,
        source: std::io::Error,
    },
}

/// The contents of the files that make up the web viewer application.
///
/// Loaded once per [`WebViewerServer`] via `WebViewerData::load`,
/// which does not exist in `disable_web_viewer_server` builds.
pub struct WebViewerData {
    index_html: Cow<'static, [u8]>,
    favicon: Cow<'static, [u8]>,
    apple_touch_icon: Cow<'static, [u8]>,
    sw_js: Cow<'static, [u8]>,
    viewer_js: Cow<'static, [u8]>,
    viewer_wasm: Cow<'static, [u8]>,
    signed_in_html: Cow<'static, [u8]>,
    signed_out_html: Cow<'static, [u8]>,
}

/// Manual impl to show the size of each file instead of dumping its contents.
impl std::fmt::Debug for WebViewerData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            index_html,
            favicon,
            apple_touch_icon,
            sw_js,
            viewer_js,
            viewer_wasm,
            signed_in_html,
            signed_out_html,
        } = self;

        let mut f = f.debug_struct("WebViewerData");
        for (name, contents) in [
            ("index_html", index_html),
            ("favicon", favicon),
            ("apple_touch_icon", apple_touch_icon),
            ("sw_js", sw_js),
            ("viewer_js", viewer_js),
            ("viewer_wasm", viewer_wasm),
            ("signed_in_html", signed_in_html),
            ("signed_out_html", signed_out_html),
        ] {
            f.field(name, &format_args!("<{} bytes>", contents.len()));
        }
        f.finish()
    }
}

#[cfg(not(disable_web_viewer_server))]
impl WebViewerData {
    /// Load the web viewer assets.
    ///
    /// If `assets_archive_path` is set, the assets are read from the given zip archive.
    /// Otherwise, the built-in assets are used:
    /// by default these are embedded into the binary at compile time,
    /// while `trailing_web_viewer` builds read them from a zip archive
    /// appended to the executable by `scripts/append_web_viewer.py`.
    /// `external_web_viewer` builds have no built-in assets at all,
    /// and fail if no archive path is given.
    pub fn load(assets_archive_path: Option<&Path>) -> Result<Self, WebViewerDataError> {
        match assets_archive_path {
            Some(path) => Self::from_archive(path),
            None => Self::builtin(),
        }
    }

    /// Load the web viewer assets from a zip archive on disk.
    ///
    /// The archive must contain the files produced by the web viewer build
    /// (`pixi run rerun-build-web`) at the archive root.
    pub fn from_archive(path: &Path) -> Result<Self, WebViewerDataError> {
        let file = std::fs::File::open(path).map_err(|source| WebViewerDataError::OpenArchive {
            path: path.to_owned(),
            source,
        })?;
        let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
            .map_err(WebViewerDataError::ParseZip)?;
        Self::from_zip(&mut zip)
    }

    /// Extract the assets from a zip archive.
    fn from_zip<R: std::io::Read + std::io::Seek>(
        zip: &mut zip::ZipArchive<R>,
    ) -> Result<Self, WebViewerDataError> {
        fn extract_file<R: std::io::Read + std::io::Seek>(
            zip: &mut zip::ZipArchive<R>,
            name: &str,
        ) -> Result<Cow<'static, [u8]>, WebViewerDataError> {
            use std::io::Read as _;

            let mut file = zip
                .by_name(name)
                .map_err(|source| WebViewerDataError::ExtractFile {
                    name: name.to_owned(),
                    source,
                })?;

            let mut contents = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut contents).map_err(|source| {
                WebViewerDataError::ReadFileContents {
                    name: name.to_owned(),
                    source,
                }
            })?;

            Ok(Cow::Owned(contents))
        }

        Ok(Self {
            index_html: extract_file(zip, "index.html")?,
            favicon: extract_file(zip, "favicon.ico")?,
            apple_touch_icon: extract_file(zip, "apple-touch-icon.png")?,
            sw_js: extract_file(zip, "sw.js")?,
            viewer_js: extract_file(zip, "re_viewer.js")?,
            viewer_wasm: extract_file(zip, "re_viewer_bg.wasm")?,
            signed_in_html: extract_file(zip, "signed-in.html")?,
            signed_out_html: extract_file(zip, "signed-out.html")?,
        })
    }

    /// The assets embedded into the binary at compile time.
    #[cfg(all(not(trailing_web_viewer), not(external_web_viewer)))]
    #[expect(clippy::large_include_file)]
    #[expect(clippy::unnecessary_wraps)] // Signature must match the `trailing_web_viewer` version.
    fn builtin() -> Result<Self, WebViewerDataError> {
        // If you add/remove/change the paths here, also update the include-list in `Cargo.toml`!
        Ok(Self {
            index_html: Cow::Borrowed(include_bytes!("../web_viewer/index.html")),
            favicon: Cow::Borrowed(include_bytes!("../web_viewer/favicon.ico")),
            apple_touch_icon: Cow::Borrowed(include_bytes!("../web_viewer/apple-touch-icon.png")),
            sw_js: Cow::Borrowed(include_bytes!("../web_viewer/sw.js")),
            viewer_js: Cow::Borrowed(include_bytes!("../web_viewer/re_viewer.js")),
            viewer_wasm: Cow::Borrowed(include_bytes!("../web_viewer/re_viewer_bg.wasm")),
            signed_in_html: Cow::Borrowed(include_bytes!("../web_viewer/signed-in.html")),
            signed_out_html: Cow::Borrowed(include_bytes!("../web_viewer/signed-out.html")),
        })
    }

    /// The assets from the zip archive appended to the executable
    /// by `scripts/append_web_viewer.py`.
    #[cfg(trailing_web_viewer)]
    fn builtin() -> Result<Self, WebViewerDataError> {
        let zip_bytes = trailing_data::read_zip_from_exe()?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
            .map_err(WebViewerDataError::ParseZip)?;
        Self::from_zip(&mut zip)
    }

    /// `external_web_viewer` builds have no built-in assets:
    /// they must be loaded from an archive on disk instead.
    #[cfg(external_web_viewer)]
    fn builtin() -> Result<Self, WebViewerDataError> {
        Err(WebViewerDataError::NoBuiltinAssets)
    }
}

impl WebViewerData {
    #[inline]
    pub fn index_html(&self) -> &[u8] {
        &self.index_html
    }

    #[inline]
    pub fn favicon(&self) -> &[u8] {
        &self.favicon
    }

    #[inline]
    pub fn apple_touch_icon(&self) -> &[u8] {
        &self.apple_touch_icon
    }

    #[inline]
    pub fn sw_js(&self) -> &[u8] {
        &self.sw_js
    }

    #[inline]
    pub fn viewer_js(&self) -> &[u8] {
        &self.viewer_js
    }

    #[inline]
    pub fn viewer_wasm(&self) -> &[u8] {
        &self.viewer_wasm
    }

    #[inline]
    pub fn signed_in_html(&self) -> &[u8] {
        &self.signed_in_html
    }

    #[inline]
    pub fn signed_out_html(&self) -> &[u8] {
        &self.signed_out_html
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Typed port for use with [`WebViewerServer`]
pub struct WebViewerServerPort(pub u16);

impl From<u16> for WebViewerServerPort {
    #[inline]
    fn from(port: u16) -> Self {
        Self(port)
    }
}

impl WebViewerServerPort {
    /// Port to use with [`WebViewerServer::new`] when you want the OS to pick a port for you.
    ///
    /// This is defined as `0`.
    pub const AUTO: Self = Self(0);
}

impl Default for WebViewerServerPort {
    fn default() -> Self {
        Self(DEFAULT_WEB_VIEWER_SERVER_PORT)
    }
}

impl Display for WebViewerServerPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Needed for clap
impl FromStr for WebViewerServerPort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u16>() {
            Ok(port) => Ok(Self(port)),
            Err(err) => Err(format!("Failed to parse port: {err}")),
        }
    }
}

/// HTTP host for the Rerun Web Viewer application
/// This serves the HTTP+Wasm+JS files that make up the web-viewer.
#[must_use = "Dropping this means stopping the server"]
pub struct WebViewerServer {
    inner: Arc<WebViewerServerInner>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

struct WebViewerServerInner {
    server: tiny_http::Server,
    shutdown: AtomicBool,
    num_wasm_served: AtomicU64,

    #[cfg(not(disable_web_viewer_server))]
    data: WebViewerData,
}

impl WebViewerServer {
    /// Create new [`WebViewerServer`] to host the Rerun Web Viewer on a specified port.
    ///
    /// [`WebViewerServerPort::AUTO`] will tell the OS choose any free port.
    ///
    /// The server will immediately start listening for incoming connections
    /// and stop doing so when the returned [`WebViewerServer`] is dropped.
    ///
    /// ## Example
    /// ``` no_run
    /// # use re_web_viewer_server::{WebViewerServer, WebViewerServerPort, WebViewerServerError};
    /// # async fn example() -> Result<(), WebViewerServerError> {
    /// let server = WebViewerServer::new("0.0.0.0", WebViewerServerPort::AUTO)?;
    /// let server_url = server.server_url();
    /// # Ok(()) }
    /// ```
    pub fn new(bind_ip: &str, port: WebViewerServerPort) -> Result<Self, WebViewerServerError> {
        Self::with_archive(bind_ip, port, None)
    }

    /// Like [`WebViewerServer::new`], but if `assets_archive_path` is set,
    /// the web viewer assets are served from the given zip archive
    /// instead of the assets built into the binary.
    pub fn with_archive(
        bind_ip: &str,
        port: WebViewerServerPort,
        assets_archive_path: Option<&Path>,
    ) -> Result<Self, WebViewerServerError> {
        // Load the assets eagerly so that e.g. a missing archive fails server
        // creation instead of killing the serve thread on the first request.
        cfg_select! {
            disable_web_viewer_server => {
                let _ = assets_archive_path;
            }
            _ => {
                let data = WebViewerData::load(assets_archive_path)?;
            }
        }

        let bind_addr = std::net::SocketAddr::new(bind_ip.parse()?, port.0);

        let server = tiny_http::Server::http(bind_addr).map_err(|err| {
            WebViewerServerError::CreateServerFailed {
                address: bind_addr.to_string(),
                source: err,
            }
        })?;
        let shutdown = AtomicBool::new(false);

        let inner = Arc::new(WebViewerServerInner {
            server,
            shutdown,
            num_wasm_served: Default::default(),
            #[cfg(not(disable_web_viewer_server))]
            data,
        });

        let inner_copy = inner.clone();

        // TODO(andreas): Should we create a bunch of worker threads as proposed by https://docs.rs/tiny_http/latest/tiny_http/#creating-the-server ?
        // Not doing this right now since what we're serving out is so trivial (just a few files).
        let thread_handle = std::thread::Builder::new()
            .name("re_web_viewer_server".to_owned())
            .spawn(move || inner_copy.serve())
            .ok();

        Ok(Self {
            inner,
            thread_handle,
        })
    }

    /// Includes `http://` prefix
    pub fn server_url(&self) -> String {
        let local_addr = self.inner.server.server_addr();
        if let Some(local_addr) = local_addr.clone().to_ip()
            && local_addr.ip().is_unspecified()
        {
            return format!("http://127.0.0.1:{}", local_addr.port());
        }
        format!("http://{local_addr}")
    }

    pub fn bound_url(&self) -> String {
        format!("http://{}", self.inner.server.server_addr())
    }

    /// Blocks execution as long as the server is running.
    ///
    /// There's no way of shutting the server down from the outside right now.
    pub fn block(mut self) {
        if let Some(thread_handle) = self.thread_handle.take() {
            thread_handle.join().ok();
        }
    }

    /// Keeps the web viewer running until the parent process shuts down.
    pub fn detach(mut self) {
        if let Some(thread_handle) = self.thread_handle.take() {
            // dropping the thread handle detaches the thread.
            drop(thread_handle);
        }
    }
}

impl Drop for WebViewerServer {
    fn drop(&mut self) {
        if let Some(thread_handle) = self.thread_handle.take() {
            let num_wasm_served = self.inner.num_wasm_served.load(Ordering::Relaxed);
            re_log::debug!(
                "Shutting down web server after serving the Wasm {num_wasm_served} time(s)"
            );

            self.inner.shutdown.store(true, Ordering::Release);
            self.inner.server.unblock();
            thread_handle.join().ok();
        }
    }
}

impl WebViewerServerInner {
    fn serve(&self) {
        loop {
            let request = self.server.recv();
            if self.shutdown.load(Ordering::Acquire) {
                return;
            }

            let request = match request {
                Ok(request) => request,
                Err(err) => {
                    re_log::error!("Failed to receive http request: {err}");
                    continue;
                }
            };

            if let Err(err) = self.send_response(request) {
                re_log::error!("Failed to send http response: {err}");
            }
        }
    }

    fn on_serve_wasm(&self) {
        self.num_wasm_served.fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "analytics")]
        re_analytics::record(|| re_analytics::event::ServeWasm);
    }

    #[cfg(disable_web_viewer_server)]
    fn send_response(&self, _request: tiny_http::Request) -> Result<(), std::io::Error> {
        if false {
            self.on_serve_wasm(); // to silence warning about the function being unused
        }
        panic!(
            "re_web_viewer_server compiled without .wasm, because of '__disable_server' feature, `--all-features`, or 'RERUN_DISABLE_WEB_VIEWER_SERVER=1'. DON'T DO THAT! It's only meant for tests and docs!"
        );
    }

    #[cfg(not(disable_web_viewer_server))]
    fn send_response(&self, request: tiny_http::Request) -> Result<(), std::io::Error> {
        // Strip arguments from url so we get the actual path.
        let url = request.url();
        let path = url.split('?').next().unwrap_or(url);

        let data = &self.data;
        let (mime, bytes): (&str, &[u8]) = match path {
            "/" | "/index.html" => ("text/html", data.index_html()),
            "/favicon.ico" => ("image/x-icon", data.favicon()),
            "/apple-touch-icon.png" => ("image/png", data.apple_touch_icon()),
            "/sw.js" => ("text/javascript", data.sw_js()),
            "/re_viewer.js" => ("text/javascript", data.viewer_js()),
            "/re_viewer_bg.wasm" => {
                self.on_serve_wasm();
                ("application/wasm", data.viewer_wasm())
            }
            "/signed-in" => ("text/html", data.signed_in_html()),
            "/signed-out" => ("text/html", data.signed_out_html()),
            _ => {
                re_log::warn!("404 path: {}", path);
                return request.respond(tiny_http::Response::empty(404));
            }
        };

        // TODO(#6061): Wasm should be compressed.

        let mut response = tiny_http::Response::from_data(bytes).with_header(
            tiny_http::Header::from_str(&format!("Content-Type: {mime}"))
                // Both `mime` and the header are hardcoded, so shouldn't be able to fail depending on user input.
                .expect("Invalid http header"),
        );

        // The wasm files are pretty large, so they'll be sent chunked (ideally we'd gzip them…).
        // (tiny_http will do so automatically if the data is above a certain threshold.
        // It is configurable, but we don't know all the implications of that.)
        // Unfortunately `Transfer-Encoding: chunked` means that no size is transmitted.
        // We work around this by adding a custom header with the size that web_viewer/index.html understands.
        if let Ok(header) =
            tiny_http::Header::from_str(&format!("rerun-final-length: {}", bytes.len()))
        {
            response.add_header(header);
        }

        request.respond(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unspecified_bind_address_has_distinct_bound_and_connect_urls() {
        let server = WebViewerServer::new("0.0.0.0", WebViewerServerPort::AUTO).unwrap();
        let port = server.inner.server.server_addr().to_ip().unwrap().port();

        assert_eq!(server.bound_url(), format!("http://0.0.0.0:{port}"));
        assert_eq!(server.server_url(), format!("http://127.0.0.1:{port}"));
    }

    #[cfg(not(disable_web_viewer_server))]
    const ASSET_FILE_NAMES: [&str; 8] = [
        "index.html",
        "favicon.ico",
        "apple-touch-icon.png",
        "sw.js",
        "re_viewer.js",
        "re_viewer_bg.wasm",
        "signed-in.html",
        "signed-out.html",
    ];

    /// Write a zip archive containing the given file names, each with its own name as contents.
    #[cfg(not(disable_web_viewer_server))]
    fn write_asset_archive(path: &Path, file_names: &[&str]) {
        use std::io::Write as _;

        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for name in file_names {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(name.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    #[cfg(not(disable_web_viewer_server))]
    fn load_data_from_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("web_viewer.zip");
        write_asset_archive(&path, &ASSET_FILE_NAMES);

        let data = WebViewerData::from_archive(&path).unwrap();
        assert_eq!(data.index_html(), b"index.html");
        assert_eq!(data.viewer_wasm(), b"re_viewer_bg.wasm");
        assert_eq!(data.signed_out_html(), b"signed-out.html");
    }

    #[test]
    #[cfg(not(disable_web_viewer_server))]
    fn archive_with_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("web_viewer.zip");
        write_asset_archive(&path, &ASSET_FILE_NAMES[..7]); // No `signed-out.html`

        let err = WebViewerData::from_archive(&path).unwrap_err();
        assert!(
            matches!(&err, WebViewerDataError::ExtractFile { name, .. } if name == "signed-out.html"),
            "unexpected error: {err}"
        );
    }

    #[test]
    #[cfg(not(disable_web_viewer_server))]
    fn missing_archive_fails() {
        let err =
            WebViewerData::from_archive(Path::new("/nonexistent/web_viewer.zip")).unwrap_err();
        assert!(
            matches!(err, WebViewerDataError::OpenArchive { .. }),
            "unexpected error: {err}"
        );
    }
}
