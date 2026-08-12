use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::types::PyBytes;
use pyo3::{Bound, PyAny, PyResult, Python, pyfunction};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_sdk_types::components::VideoCodec;
use re_video::VideoLoadError;

use crate::arrow::array_to_rust;

/// `fourcc` is a `rerun.components.VideoCodec` enum value from Python;
/// reuse the canonical fourcc→codec conversion rather than re-mapping here.
fn codec_from_fourcc(fourcc: u32) -> PyResult<re_video::VideoCodec> {
    Ok(VideoCodec::try_from_u32(fourcc)
        .ok_or_else(|| {
            PyValueError::new_err(format!("Unknown video codec fourcc: {fourcc:#010x}"))
        })?
        .into())
}

/// Detect whether a video sample starts a group of pictures, i.e. is a keyframe.
///
/// H.264/H.265 samples must be in Annex B format.
/// `codec_fourcc` is a `rerun.components.VideoCodec` enum value.
#[pyfunction]
#[pyo3(signature = (sample, codec_fourcc))]
pub fn video_detect_gop_start(sample: &[u8], codec_fourcc: u32) -> PyResult<bool> {
    match re_video::detect_gop_start(sample, codec_from_fourcc(codec_fourcc)?) {
        Ok(re_video::GopStartDetection::StartOfGop(_)) => Ok(true),
        Ok(re_video::GopStartDetection::NotStartOfGop) => Ok(false),
        Err(err) => Err(PyValueError::new_err(err.to_string())),
    }
}

/// Convert a length-prefixed (AVCC-style) NAL unit sample to Annex B (start-code-prefixed).
#[pyfunction]
#[pyo3(signature = (sample, length_prefix_size = 4))]
pub fn video_length_prefixed_to_annex_b<'py>(
    py: Python<'py>,
    sample: &[u8],
    length_prefix_size: usize,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut annex_b = Vec::with_capacity(sample.len() + 16);
    re_video::write_length_prefixed_nalus_to_annexb_stream(
        &mut annex_b,
        sample,
        length_prefix_size,
    )
    .map_err(|err| PyValueError::new_err(err.to_string()))?;
    Ok(PyBytes::new(py, &annex_b))
}

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

    let video_bytes_arrow_uint8_array = video_bytes_arrow_array
        .downcast_array_ref::<arrow::array::ListArray>()
        .and_then(|arr| arr.values().downcast_array_ref::<arrow::array::UInt8Array>())
        .ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "Expected arrow array to be a list with a single uint8 array, instead it has the datatype {}",
                video_bytes_arrow_array.data_type()
            ))
        })?;

    let video_bytes = video_bytes_arrow_uint8_array.values().as_ref();

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
