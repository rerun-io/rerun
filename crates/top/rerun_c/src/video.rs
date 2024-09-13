use crate::{CError, CErrorCode, CStringView};

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn rr_video_asset_read_frame_timestamps_ns(
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
    let Some(alloc_func) = alloc_func else {
        CError::unexpected_null("alloc_func").write_error(error);
        return std::ptr::null_mut();
    };

    let video_bytes = unsafe { std::slice::from_raw_parts(video_bytes, video_bytes_len as usize) };
    let media_type_str = media_type.as_str("media_type").ok();

    let video = match re_video::VideoData::load_from_bytes(video_bytes, media_type_str) {
        Ok(video) => video,
        Err(err) => {
            CError::new(
                CErrorCode::VideoLoadError,
                &format!("Failed to load video data: {err}"),
            )
            .write_error(error);
            return std::ptr::null_mut();
        }
    };

    // TODO(andreas): Producing this iterator isn't super expensive, but an ExactSizeIterator would be good to avoid
    // the somewhat brittle size-oracle here!
    // (note that since we create a slice from the allocation, this won't be able to go out of bound even if this value is too small)
    let num_timestamps = video.segments.iter().map(|s| s.samples.len()).sum();
    let timestamps_ns_memory = alloc_func(alloc_context, num_timestamps as u32);
    let timestamps_ns =
        unsafe { std::slice::from_raw_parts_mut(timestamps_ns_memory, num_timestamps) };
    for (segment, timestamp_ns) in video.frame_timestamps_ns().zip(timestamps_ns.iter_mut()) {
        *timestamp_ns = segment;
    }

    timestamps_ns.as_mut_ptr()
}
