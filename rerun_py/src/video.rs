#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use pyo3::{exceptions::PyRuntimeError, pyfunction, Bound, PyAny, PyResult};

use re_arrow_util::Arrow2ArrayDowncastRef as _;
use re_sdk::ComponentDescriptor;
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
pub fn asset_video_read_frame_timestamps_ns(
    video_bytes_arrow_array: &Bound<'_, PyAny>,
    media_type: Option<&str>,
) -> PyResult<Vec<i64>> {
    let component_descr = ComponentDescriptor {
        archetype_name: Some("rerun.archetypes.AssetVideo".into()),
        archetype_field_name: Some("blob".into()),
        component_name: "rerun.components.Blob".into(),
    };
    let video_bytes_arrow_array = array_to_rust(video_bytes_arrow_array, &component_descr)?.0;

    let video_bytes_arrow_uint8_array = video_bytes_arrow_array
        .downcast_array2_ref::<arrow2::array::ListArray<i32>>()
        .and_then(|arr| arr.values().downcast_array2_ref::<arrow2::array::UInt8Array>())
        .ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Expected arrow array to be a list with a single uint8 array, instead it has the datatype {:?}",
                video_bytes_arrow_array.data_type()
            ))
        })?;

    let video_bytes = video_bytes_arrow_uint8_array.values().as_slice();

    let Some(media_type) =
        media_type.or_else(|| infer::Infer::new().get(video_bytes).map(|v| v.mime_type()))
    else {
        return Err(PyRuntimeError::new_err(
            VideoLoadError::UnrecognizedMimeType.to_string(),
        ));
    };

    Ok(
        re_video::VideoData::load_from_bytes(video_bytes, media_type)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
            .frame_timestamps_ns()
            .collect(),
    )
}
