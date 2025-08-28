#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use pyo3::{Bound, PyAny, PyResult, exceptions::PyRuntimeError, pyfunction};

use re_chunk::ArrowArray as _;
use re_video::VideoLoadError;

use crate::arrow::array_to_rust;

/// Reads the timestamps of all frames in a video asset.
///
/// Implementation note:
/// On the Python side we start out with a pyarrow array of bytes. Converting it to
/// Python `bytes` can be done with `to_pybytes` but this requires copying the data.
/// So instead, we pass the arrow array directly.
#[pyfunction]
#[pyo3(signature = (video_bytes_arrow_array, media_type=None))]
pub fn asset_video_read_frame_timestamps_nanos(
    video_bytes_arrow_array: &Bound<'_, PyAny>,
    media_type: Option<&str>,
) -> PyResult<Vec<i64>> {
    let video_bytes_arrow_array = array_to_rust(video_bytes_arrow_array)?;
    let video_bytes = binary_array_as_slice(&video_bytes_arrow_array).ok_or_else(|| {
        PyRuntimeError::new_err(format!(
            "Expected video bytes to be a single BinaryArray, instead it has the datatype {:?} x {}",
            video_bytes_arrow_array.data_type(),
            video_bytes_arrow_array.len(),
        ))
    })?;
    let Some(media_type) =
        media_type.or_else(|| infer::Infer::new().get(video_bytes).map(|v| v.mime_type()))
    else {
        return Err(PyRuntimeError::new_err(
            VideoLoadError::UnrecognizedMimeType.to_string(),
        ));
    };

    Ok(
        re_video::VideoDataDescription::load_from_bytes(video_bytes, media_type, "AssetVideo")
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
            .frame_timestamps_nanos()
            .ok_or_else(|| PyRuntimeError::new_err(VideoLoadError::NoTimescale.to_string()))?
            .collect(),
    )
}

fn binary_array_as_slice(array: &std::sync::Arc<dyn arrow::array::Array>) -> Option<&[u8]> {
    if let Some(blob_data) = array.as_any().downcast_ref::<arrow::array::BinaryArray>() {
        if blob_data.len() == 1 {
            return Some(blob_data.value(0));
        }
    }

    if let Some(blob_data) = array
        .as_any()
        .downcast_ref::<arrow::array::LargeBinaryArray>()
    {
        if blob_data.len() == 1 {
            return Some(blob_data.value(0));
        }
    }

    None
}
