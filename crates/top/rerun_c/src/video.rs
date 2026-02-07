use crate::{CError, CErrorCode, CStringView};

#[expect(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn rr_video_asset_read_frame_timestamps_nanos(
    video_bytes: *const u8,
    video_bytes_len: u64,
    media_type: CStringView,
    alloc_context: *mut std::ffi::c_void,
    alloc_func: Option<
        extern "C" fn(context: *mut std::ffi::c_void, num_timestamps: u32) -> *mut i64,
    >,
    error: *mut CError,
) -> *mut i64 {
    if video_bytes.is_null() {
        CError::unexpected_null("video_bytes").write_error(error);
        return std::ptr::null_mut();
    }
    if video_bytes_len == 0 {
        CError::new(CErrorCode::VideoLoadError, "Zero video bytes").write_error(error);
        return std::ptr::null_mut();
    }
    let Some(alloc_func) = alloc_func else {
        CError::unexpected_null("alloc_func").write_error(error);
        return std::ptr::null_mut();
    };

    let video_bytes = unsafe { std::slice::from_raw_parts(video_bytes, video_bytes_len as usize) };
    let media_type_str = media_type.as_maybe_empty_str("media_type").ok();

    let Some(media_type_str) =
        media_type_str.or_else(|| infer::Infer::new().get(video_bytes).map(|v| v.mime_type()))
    else {
        CError::new(
            CErrorCode::VideoLoadError,
            &re_video::VideoLoadError::UnrecognizedMimeType.to_string(),
        )
        .write_error(error);
        return std::ptr::null_mut();
    };

    let video = match re_video::VideoDataDescription::load_from_bytes(
        video_bytes,
        media_type_str,
        "AssetVideo",
        re_sdk::external::re_tuid::Tuid::new(),
    ) {
        Ok(video) => video,
        Err(err) => {
            CError::new(
                CErrorCode::VideoLoadError,
                &format!("Failed to load video: {err}"),
            )
            .write_error(error);
            return std::ptr::null_mut();
        }
    };

    let num_timestamps = video.samples.num_elements();
    let timestamps_nanos_memory = alloc_func(alloc_context, num_timestamps as u32);
    let timestamps_nanos =
        unsafe { std::slice::from_raw_parts_mut(timestamps_nanos_memory, num_timestamps) };

    let Some(video_timestamps_iter) = video.frame_timestamps_nanos() else {
        CError::new(
            CErrorCode::VideoLoadError,
            &re_video::VideoLoadError::NoTimescale.to_string(),
        )
        .write_error(error);
        return std::ptr::null_mut();
    };

    for (segment, timestamp_nanos) in video_timestamps_iter.zip(timestamps_nanos.iter_mut()) {
        *timestamp_nanos = segment;
    }

    timestamps_nanos.as_mut_ptr()
}
