//! Reading of the zip archive appended to the executable.
//!
//! This module is used in `trailing_web_viewer` builds (`RERUN_TRAILING_WEB_VIEWER=1`),
//! where the web viewer assets are appended to the binary in a post-processing step
//! using `scripts/append_web_viewer.py`.
//!
//! Format of trailing data:
//! ```text
//! [Original Binary] [ZIP Archive] [ZIP Offset: 8 bytes LE] [Magic: "RERUNWEB"]
//! ```

use std::io::{Read as _, Seek as _};

use crate::WebViewerDataError;

/// Magic marker at the end of the binary to identify the trailing data.
const MAGIC: &[u8] = b"RERUNWEB";
const MAGIC_LEN: usize = 8;
const OFFSET_LEN: usize = 8;
const TRAILER_LEN: usize = MAGIC_LEN + OFFSET_LEN;

/// Read the raw bytes of the zip archive appended to the current executable.
pub fn read_zip_from_exe() -> Result<Vec<u8>, WebViewerDataError> {
    let exe_path = std::env::current_exe().map_err(WebViewerDataError::CurrentExe)?;

    let mut file =
        std::fs::File::open(&exe_path).map_err(|source| WebViewerDataError::OpenFile {
            path: exe_path.clone(),
            source,
        })?;

    // Read the trailer (last TRAILER_LEN bytes)
    let trailer_len_i64: i64 = TRAILER_LEN
        .try_into()
        .expect("TRAILER_LEN should fit in i64");

    file.seek(std::io::SeekFrom::End(-trailer_len_i64))
        .map_err(WebViewerDataError::ReadTrailer)?;

    let mut trailer = [0u8; TRAILER_LEN];
    file.read_exact(&mut trailer)
        .map_err(WebViewerDataError::ReadTrailer)?;

    // Verify magic
    let magic = &trailer[OFFSET_LEN..];
    if magic != MAGIC {
        return Err(WebViewerDataError::InvalidMagic {
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
        .map_err(|source| WebViewerDataError::SeekToZip {
            offset: zip_offset,
            source,
        })?;

    // Calculate the zip size (excluding the trailer)
    let file_size = file
        .metadata()
        .map_err(WebViewerDataError::ExeMetadata)?
        .len();
    let zip_size = file_size - zip_offset - TRAILER_LEN as u64;

    // Read the zip archive
    let mut zip_data = Vec::with_capacity(zip_size as usize);
    file.take(zip_size)
        .read_to_end(&mut zip_data)
        .map_err(|source| WebViewerDataError::ReadZip {
            size: zip_size,
            source,
        })?;

    Ok(zip_data)
}
