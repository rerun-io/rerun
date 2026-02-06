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
    load_from_trailing_zip().expect(
        "Failed to load web viewer from trailing data. \
        This binary was built with __trailing_web_viewer feature, which requires \
        a post-processing step to append the web viewer assets. \
        See re_web_viewer_server documentation for details.",
    )
}

/// Read and parse the trailing zip archive from the current executable.
fn load_from_trailing_zip() -> Result<WebViewerData, Box<dyn std::error::Error>> {
    // Get the path to the current executable
    let exe_path = std::env::current_exe()?;
    let mut file = std::fs::File::open(&exe_path)?;

    // Read the trailer (last TRAILER_LEN bytes)
    let trailer_len_i64: i64 = TRAILER_LEN.try_into()?;
    file.seek(std::io::SeekFrom::End(-trailer_len_i64))?;
    let mut trailer = [0u8; TRAILER_LEN];
    file.read_exact(&mut trailer)?;

    // Verify magic
    let magic = &trailer[OFFSET_LEN..];
    if magic != MAGIC {
        return Err(format!(
            "Invalid magic in trailing data. Expected {MAGIC:?}, got {magic:?}. \
            This binary was built with __trailing_web_viewer but the post-processing \
            step has not been completed."
        )
        .into());
    }

    // Read the zip offset
    let zip_offset = u64::from_le_bytes(trailer[..OFFSET_LEN].try_into()?);

    // Seek to the start of the zip archive
    file.seek(std::io::SeekFrom::Start(zip_offset))?;

    // Calculate the zip size (excluding the trailer)
    let file_size = file.metadata()?.len();
    let zip_size = file_size - zip_offset - TRAILER_LEN as u64;

    // Read the zip archive
    let mut zip_data = Vec::with_capacity(zip_size as usize);
    file.take(zip_size).read_to_end(&mut zip_data)?;

    // Parse the zip archive
    let cursor = std::io::Cursor::new(zip_data);
    let mut zip = zip::ZipArchive::new(cursor)?;

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
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut file = zip.by_name(name)?;
    let mut contents = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut contents)?;
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
