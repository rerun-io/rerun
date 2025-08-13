#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use pyo3::{Bound, PyAny, PyResult, exceptions::PyRuntimeError, pyfunction};

use re_arrow_util::ArrowArrayDowncastRef as _;
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

    let video_bytes_arrow_binary_array = video_bytes_arrow_array
        .downcast_array_ref::<arrow::array::BinaryArray>()
        .ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Expected video bytes to be BinaryArray, instead it has the datatype {:?}",
                video_bytes_arrow_array.data_type()
            ))
        })?;

    if video_bytes_arrow_binary_array.len() != 1 {
        return Err(PyRuntimeError::new_err(format!(
            "Expected exactly one video file; got {}",
            video_bytes_arrow_binary_array.len()
        )));
    }

    let video_bytes = video_bytes_arrow_binary_array.value(0);

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
