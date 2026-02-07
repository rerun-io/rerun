//! Web viewer data loaded from trailing zip archive appended to the binary.
//!
//! This module is used when the `__trailing_web_viewer` feature is enabled.
//! It reads the web viewer assets from a zip archive that has been appended
//! to the end of the binary via a post-processing step.
//!
//! Format of trailing data:
//! ```text
//! [Original Binary] [ZIP Archive] [ZIP Offset: 8 bytes LE] [Magic: "RERUNWEB"]
//! ```

use std::io::{Read, Seek};
use std::sync::OnceLock;

/// Magic marker at the end of the binary to identify the trailing data.
const MAGIC: &[u8] = b"RERUNWEB";
const MAGIC_LEN: usize = 8;
const OFFSET_LEN: usize = 8;
const TRAILER_LEN: usize = MAGIC_LEN + OFFSET_LEN;

/// Errors that can occur when loading web viewer data from trailing zip.
#[derive(thiserror::Error, Debug)]
enum TrailingDataError {
    #[error("Failed to get current executable path: {0}")]
    CurrentExe(#[from] std::io::Error),

    #[error("Failed to open executable at {path:?}: {source}")]
    OpenFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

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

    #[error("Failed to parse zip archive: {0}. The appended data may be corrupted.")]
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

/// Cached web viewer data loaded from the trailing zip.
struct WebViewerData {
    index_html: Vec<u8>,
    favicon: Vec<u8>,
    sw_js: Vec<u8>,
    viewer_js: Vec<u8>,
    viewer_wasm: Vec<u8>,
    signed_in_html: Vec<u8>,
}

static WEB_VIEWER_DATA: OnceLock<WebViewerData> = OnceLock::new();

/// Load the web viewer data from the trailing zip archive.
fn load_web_viewer_data() -> WebViewerData {
    load_from_trailing_zip().unwrap_or_else(|e| {
        panic!(
            "Failed to load web viewer from trailing data: {e}\n\n\
            This binary was built with RERUN_TRAILING_WEB_VIEWER=1, which requires \
            running scripts/append_web_viewer.py to append the web viewer assets. \
            See re_web_viewer_server documentation for details."
        )
    })
}

/// Read and parse the trailing zip archive from the current executable.
fn load_from_trailing_zip() -> Result<WebViewerData, TrailingDataError> {
    // Get the path to the current executable
    let exe_path = std::env::current_exe()?;

    let mut file =
        std::fs::File::open(&exe_path).map_err(|source| TrailingDataError::OpenFile {
            path: exe_path.clone(),
            source,
        })?;

    // Read the trailer (last TRAILER_LEN bytes)
    let trailer_len_i64: i64 = TRAILER_LEN
        .try_into()
        .expect("TRAILER_LEN should fit in i64");

    file.seek(std::io::SeekFrom::End(-trailer_len_i64))
        .map_err(TrailingDataError::ReadTrailer)?;

    let mut trailer = [0u8; TRAILER_LEN];
    file.read_exact(&mut trailer)
        .map_err(TrailingDataError::ReadTrailer)?;

    // Verify magic
    let magic = &trailer[OFFSET_LEN..];
    if magic != MAGIC {
        return Err(TrailingDataError::InvalidMagic {
            expected: MAGIC,
            actual: magic.to_vec(),
        });
    }

    // Read the zip offset
    let zip_offset = u64::from_le_bytes(
        trailer[..OFFSET_LEN]
            .try_into()
            .expect("OFFSET_LEN should be 8 bytes"),
    );

    // Seek to the start of the zip archive
    file.seek(std::io::SeekFrom::Start(zip_offset))
        .map_err(|source| TrailingDataError::SeekToZip {
            offset: zip_offset,
            source,
        })?;

    // Calculate the zip size (excluding the trailer)
    let file_size = file.metadata()?.len();
    let zip_size = file_size - zip_offset - TRAILER_LEN as u64;

    // Read the zip archive
    let mut zip_data = Vec::with_capacity(zip_size as usize);
    file.take(zip_size)
        .read_to_end(&mut zip_data)
        .map_err(|source| TrailingDataError::ReadZip {
            size: zip_size,
            source,
        })?;

    // Parse the zip archive
    let cursor = std::io::Cursor::new(zip_data);
    let mut zip = zip::ZipArchive::new(cursor).map_err(TrailingDataError::ParseZip)?;

    // Extract each file
    let index_html = extract_file(&mut zip, "index.html")?;
    let favicon = extract_file(&mut zip, "favicon.svg")?;
    let sw_js = extract_file(&mut zip, "sw.js")?;
    let viewer_js = extract_file(&mut zip, "re_viewer.js")?;
    let viewer_wasm = extract_file(&mut zip, "re_viewer_bg.wasm")?;
    let signed_in_html = extract_file(&mut zip, "signed-in.html")?;

    Ok(WebViewerData {
        index_html,
        favicon,
        sw_js,
        viewer_js,
        viewer_wasm,
        signed_in_html,
    })
}

/// Extract a single file from the zip archive.
fn extract_file<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, TrailingDataError> {
    let mut file = zip
        .by_name(name)
        .map_err(|source| TrailingDataError::ExtractFile {
            name: name.to_owned(),
            source,
        })?;

    let mut contents = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut contents)
        .map_err(|source| TrailingDataError::ReadFileContents {
            name: name.to_owned(),
            source,
        })?;

    Ok(contents)
}

/// Get a reference to the cached web viewer data.
fn get_data() -> &'static WebViewerData {
    WEB_VIEWER_DATA.get_or_init(load_web_viewer_data)
}

// Public accessor functions that return static byte slices.
// These match the interface of the `data` module.

#[inline]
pub fn index_html() -> &'static [u8] {
    &get_data().index_html
}

#[inline]
pub fn favicon() -> &'static [u8] {
    &get_data().favicon
}

#[inline]
pub fn sw_js() -> &'static [u8] {
    &get_data().sw_js
}

#[inline]
pub fn viewer_js() -> &'static [u8] {
    &get_data().viewer_js
}

#[inline]
pub fn viewer_wasm() -> &'static [u8] {
    &get_data().viewer_wasm
}

#[inline]
pub fn signed_in_html() -> &'static [u8] {
    &get_data().signed_in_html
}
