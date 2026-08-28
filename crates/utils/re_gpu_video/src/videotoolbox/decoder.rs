//! The `VideoToolbox` decoder: a `VTDecompressionSession` fed one sample buffer per
//! access unit, emitting frames through its output callback.
//!
//! Unlike the Vulkan backend there is no parser and no DPB: the session is built from
//! the stream's parameter sets and does all of that itself. What it does not do is
//! reorder, `kVTDecodeFrame_EnableTemporalProcessing` is permission to delay frames
//! rather than a request to sort them, so frames come out in decoding order and the
//! public decoder's reorder buffer puts them back in order, keyed on their timestamps.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;

use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, Type as _,
};
use objc2_core_media::{
    CMBlockBuffer, CMFormatDescription, CMSampleBuffer, CMSampleTimingInfo, CMTime, CMTimeFlags,
    CMVideoFormatDescriptionCreateFromH264ParameterSets,
    CMVideoFormatDescriptionCreateFromHEVCParameterSets, kCMBlockBufferAssureMemoryNowFlag,
};
use objc2_core_video::{
    CVImageBuffer, kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use objc2_video_toolbox::{
    VTDecodeFrameFlags, VTDecodeInfoFlags, VTDecompressionOutputCallbackRecord,
    VTDecompressionSession,
};
use parking_lot::Mutex;

use crate::{Codec, DecodeError, DecodedFrame, ParseError};

use super::PixelBuffer;
use super::nalu::AccessUnitSplitter;

/// How many frames the session may be working on before one comes back, on top of
/// the stream's own reorder depth.
///
/// Decoding is asynchronous, so a frame's callback can land a few pushes after the
/// one that submitted it.
const PIPELINE_DEPTH: usize = 4;

/// Upper bound on the per-frame bookkeeping waiting for frames to come out,
/// far above the handful of frames that are ever in flight at once.
const MAX_PENDING_FRAMES: usize = 256;

/// Decodes access units of one codec into wgpu textures, via `VideoToolbox`.
pub struct VideoToolboxDecoder {
    wgpu_device: wgpu::Device,
    codec: Codec,

    splitter: AccessUnitSplitter,

    /// Recreated whenever the parameter sets change.
    session: Option<Session>,

    /// Decoded pixel buffers the output callback delivered, not yet wrapped as textures.
    decoded: Vec<DecodedPicture>,

    /// Whether each pushed access unit held a random access point, by timestamp,
    /// waiting for its decoded frame to come out.
    pending_is_idr: BTreeMap<i64, bool>,

    /// Set after a reset or an error: anything until the next random access point
    /// would decode against references that are gone.
    waiting_for_random_access: bool,
}

impl VideoToolboxDecoder {
    pub fn new(wgpu_device: wgpu::Device, codec: Codec) -> Result<Self, DecodeError> {
        if !matches!(codec, Codec::H264 | Codec::H265) {
            return Err(DecodeError::UnsupportedCodec(codec));
        }

        Ok(Self {
            wgpu_device,
            codec,
            splitter: AccessUnitSplitter::new(codec),
            session: None,
            decoded: Vec::new(),
            pending_is_idr: BTreeMap::new(),
            waiting_for_random_access: true,
        })
    }

    /// See [`crate::Decoder::push_access_unit`].
    pub fn push_access_unit(
        &mut self,
        data: &[u8],
        pts: i64,
    ) -> Result<Vec<(i64, DecodedFrame)>, DecodeError> {
        re_tracing::profile_function!();

        if let Err(err) = self.decode(data, pts) {
            self.waiting_for_random_access = true;
            return Err(err);
        }
        self.collect_frames()
    }

    /// See [`crate::Decoder::flush`].
    pub fn flush(&mut self) -> Result<Vec<(i64, DecodedFrame)>, DecodeError> {
        if let Some(session) = &self.session {
            session.wait_for_frames()?;
        }
        self.collect_frames()
    }

    /// See [`crate::Decoder::reset`].
    pub fn reset(&mut self) {
        if let Some(session) = &self.session {
            // The delayed frames belong to the stream position we are leaving.
            let _drained = session.wait_for_frames();
            session.take_frames();
        }
        self.decoded.clear();
        self.splitter.reset();
        self.pending_is_idr.clear();
        self.waiting_for_random_access = true;
    }

    /// `max_num_reorder_frames` of the active sequence parameter set: how many
    /// frames the reorder buffer has to hold.
    pub fn reorder_delay(&self) -> usize {
        self.splitter.reorder_depth()
    }

    /// See [`PIPELINE_DEPTH`].
    #[expect(clippy::unused_self, reason = "mirrors the Vulkan decoder")]
    pub fn pipeline_depth(&self) -> usize {
        PIPELINE_DEPTH
    }

    fn decode(&mut self, data: &[u8], pts: i64) -> Result<(), DecodeError> {
        let unit = self.splitter.split(data)?;

        if self.waiting_for_random_access {
            if !unit.is_random_access {
                return Ok(());
            }
            self.waiting_for_random_access = false;
        }

        if unit.parameters_changed
            && let Some(session) = self.session.take()
        {
            // Frames still in flight belong to the old session, drain before dropping it.
            session.wait_for_frames()?;
            self.decoded.extend(session.take_frames());
        }
        if self.session.is_none() {
            let Some(parameter_sets) = self.splitter.parameters().in_order() else {
                return Err(ParseError::MissingReference {
                    what: "parameter sets before the first frame",
                }
                .into());
            };
            self.session = Some(Session::new(self.codec, &parameter_sets)?);
        }

        if unit.sample_data.is_empty() {
            // An access unit carrying nothing but parameter sets.
            return Ok(());
        }

        self.pending_is_idr.insert(pts, unit.is_random_access);
        // Entries leave as their frame comes out, a few pushes later, so the map
        // holds the frames in flight rather than a whole group of pictures. The
        // exception is a frame the decoder drops, which never claims its entry:
        // this is the backstop against those adding up.
        while self.pending_is_idr.len() > MAX_PENDING_FRAMES {
            self.pending_is_idr.pop_first();
        }
        self.session
            .as_ref()
            .expect("created above")
            .decode(&unit.sample_data, pts)
    }

    /// Wraps everything the output callback delivered so far as textures, keyed by
    /// the timestamp their presentation order follows.
    fn collect_frames(&mut self) -> Result<Vec<(i64, DecodedFrame)>, DecodeError> {
        if let Some(session) = &self.session {
            session.check_error()?;
            self.decoded.extend(session.take_frames());
        }

        let mut out = Vec::with_capacity(self.decoded.len());
        for picture in std::mem::take(&mut self.decoded) {
            let is_idr = self.pending_is_idr.remove(&picture.pts).unwrap_or(false);

            let frame = super::output::wrap(
                &self.wgpu_device,
                &picture.pixel_buffer,
                picture.pts,
                is_idr,
            )?;
            out.push((picture.pts, frame));
        }
        Ok(out)
    }
}

/// One frame handed over by the output callback.
struct DecodedPicture {
    pixel_buffer: PixelBuffer,
    pts: i64,
}

/// Where the output callback puts its frames, shared with `VideoToolbox`'s decode threads.
#[derive(Default)]
struct FrameSink {
    frames: Mutex<Vec<DecodedPicture>>,

    /// The first status the callback reported a failure with.
    error: Mutex<Option<i32>>,
}

/// A `VTDecompressionSession` together with the format description it was built for
/// and the sink its callback writes to.
struct Session {
    /// Invalidated on drop, which is also what stops the callbacks.
    session: CFRetained<VTDecompressionSession>,

    format: CFRetained<CMFormatDescription>,

    /// The callback holds a pointer into this allocation, so it must outlive the session.
    sink: Arc<FrameSink>,
}

// SAFETY: `VTDecompressionSession`, `CMFormatDescription` and `CVPixelBuffer` are all
// reference counted CoreFoundation types with thread safe retain/release, and
// `VideoToolbox` is documented to be called from any thread.
#[expect(unsafe_code)]
unsafe impl Send for Session {}

// SAFETY: See `Session`. The pixel buffers a `FrameSink` holds are only read.
#[expect(unsafe_code)]
unsafe impl Send for FrameSink {}
// SAFETY: See `FrameSink`. Everything in it sits behind a mutex.
#[expect(unsafe_code)]
unsafe impl Sync for FrameSink {}

impl Drop for Session {
    fn drop(&mut self) {
        #[expect(unsafe_code)]
        // SAFETY: The session is still alive here, and after invalidation no further
        // callback can reach the sink this drops right after.
        unsafe {
            self.session.invalidate();
        }
    }
}

impl Session {
    fn new(codec: Codec, parameter_sets: &[&[u8]]) -> Result<Self, DecodeError> {
        re_tracing::profile_function!();

        let format = create_format_description(codec, parameter_sets)?;
        let attributes = destination_image_buffer_attributes();
        let sink = Arc::new(FrameSink::default());

        let callback = VTDecompressionOutputCallbackRecord {
            decompressionOutputCallback: Some(output_callback),
            decompressionOutputRefCon: Arc::as_ptr(&sink).cast::<c_void>().cast_mut(),
        };

        let mut raw: *mut VTDecompressionSession = std::ptr::null_mut();
        #[expect(unsafe_code)]
        // SAFETY: The format description and attributes outlive the call, the attributes
        // hold the key types CoreVideo documents, the callback record is valid and its
        // refcon points at the `sink` that this `Session` keeps alive for as long as the
        // session, and `raw` is a valid out pointer.
        let status = unsafe {
            VTDecompressionSession::create(
                None,
                &format,
                None,
                Some(&attributes),
                &raw const callback,
                NonNull::from(&mut raw),
            )
        };
        let Some(raw) = NonNull::new(raw).filter(|_| status == 0) else {
            return Err(DecodeError::VideoToolbox {
                what: "failed to create a decompression session",
                status,
            });
        };

        Ok(Self {
            // SAFETY: `VTDecompressionSessionCreate` returns an owned reference.
            #[expect(unsafe_code)]
            session: unsafe { CFRetained::from_raw(raw) },
            format,
            sink,
        })
    }

    fn decode(&self, sample_data: &[u8], pts: i64) -> Result<(), DecodeError> {
        re_tracing::profile_function!();

        let block = create_block_buffer(sample_data)?;
        let timing = CMSampleTimingInfo {
            duration: INVALID_TIME,
            presentationTimeStamp: cm_time(pts),
            decodeTimeStamp: INVALID_TIME,
        };
        let sizes = [sample_data.len()];

        let mut raw: *mut CMSampleBuffer = std::ptr::null_mut();
        #[expect(unsafe_code)]
        // SAFETY: The block buffer, format description, timing and size arrays all
        // outlive the call, their counts match the arrays, and `raw` is a valid out
        // pointer.
        let status = unsafe {
            CMSampleBuffer::create_ready(
                None,
                Some(&block),
                Some(&self.format),
                1,
                1,
                &raw const timing,
                1,
                sizes.as_ptr(),
                NonNull::from(&mut raw),
            )
        };
        let Some(raw) = NonNull::new(raw).filter(|_| status == 0) else {
            return Err(DecodeError::VideoToolbox {
                what: "failed to build a sample buffer",
                status,
            });
        };
        #[expect(unsafe_code)]
        // SAFETY: `CMSampleBufferCreateReady` returns an owned reference.
        let sample = unsafe { CFRetained::from_raw(raw) };

        // Asynchronous decoding lets the session work on several frames at once,
        // their callbacks land during later pushes.
        let flags = VTDecodeFrameFlags::Frame_EnableAsynchronousDecompression;

        #[expect(unsafe_code)]
        // SAFETY: The sample buffer outlives the call, and both the source frame
        // reference and the info flags out pointer are allowed to be null.
        let status = unsafe {
            self.session
                .decode_frame(&sample, flags, std::ptr::null_mut(), std::ptr::null_mut())
        };
        if status != 0 {
            return Err(DecodeError::VideoToolbox {
                what: "failed to decode a frame",
                status,
            });
        }

        Ok(())
    }

    /// Blocks until every frame handed to the session came back through the callback.
    fn wait_for_frames(&self) -> Result<(), DecodeError> {
        re_tracing::profile_function!();

        #[expect(unsafe_code)]
        // SAFETY: The session is alive for the duration of the call.
        let status = unsafe { self.session.wait_for_asynchronous_frames() };
        if status != 0 {
            return Err(DecodeError::VideoToolbox {
                what: "failed to wait for the outstanding frames",
                status,
            });
        }
        Ok(())
    }

    fn take_frames(&self) -> Vec<DecodedPicture> {
        std::mem::take(&mut self.sink.frames.lock())
    }

    fn check_error(&self) -> Result<(), DecodeError> {
        match self.sink.error.lock().take() {
            Some(status) => Err(DecodeError::VideoToolbox {
                what: "the decoder reported a failed frame",
                status,
            }),
            None => Ok(()),
        }
    }
}

/// The output callback `VideoToolbox` delivers decoded frames through.
///
/// # Safety
///
/// `output_ref_con` must point at a live [`FrameSink`], which is what
/// [`Session`] guarantees for as long as its session can call this.
#[expect(unsafe_code)]
unsafe extern "C-unwind" fn output_callback(
    output_ref_con: *mut c_void,
    _source_frame_ref_con: *mut c_void,
    status: i32,
    info_flags: VTDecodeInfoFlags,
    image_buffer: *mut CVImageBuffer,
    presentation_timestamp: CMTime,
    _presentation_duration: CMTime,
) {
    // SAFETY: See this function's contract.
    let sink = unsafe { &*output_ref_con.cast::<FrameSink>() };

    if status != 0 {
        *sink.error.lock() = Some(status);
        return;
    }
    if info_flags.contains(VTDecodeInfoFlags::FrameDropped) {
        return;
    }
    let Some(image_buffer) = NonNull::new(image_buffer) else {
        return;
    };
    if !presentation_timestamp.flags.contains(CMTimeFlags::Valid) {
        return;
    }

    // SAFETY: The callback contract says the buffer is valid for the call, and
    // `CFRetained::retain` takes its own reference to keep it alive beyond that.
    // `CVPixelBuffer` and `CVImageBuffer` are one and the same type.
    let pixel_buffer = unsafe { CFRetained::retain(image_buffer) };

    sink.frames.lock().push(DecodedPicture {
        pixel_buffer: PixelBuffer::new(pixel_buffer),
        // The timestamp is the one `decode` handed in, on a timescale of one.
        pts: presentation_timestamp.value,
    });
}

const INVALID_TIME: CMTime = CMTime {
    value: 0,
    timescale: 0,
    flags: CMTimeFlags::empty(),
    epoch: 0,
};

/// The caller's timestamp as a `CMTime`, on a timescale of one so it survives the
/// round trip through `VideoToolbox` unchanged.
fn cm_time(pts: i64) -> CMTime {
    CMTime {
        value: pts,
        timescale: 1,
        flags: CMTimeFlags::Valid,
        epoch: 0,
    }
}

fn create_format_description(
    codec: Codec,
    parameter_sets: &[&[u8]],
) -> Result<CFRetained<CMFormatDescription>, DecodeError> {
    let pointers: Vec<NonNull<u8>> = parameter_sets
        .iter()
        .map(|set| NonNull::from(*set).cast::<u8>())
        .collect();
    let sizes: Vec<usize> = parameter_sets.iter().map(|set| set.len()).collect();
    let Some(pointers_start) = NonNull::new(pointers.as_ptr().cast_mut()) else {
        return Err(ParseError::MissingReference {
            what: "parameter sets",
        }
        .into());
    };

    let mut raw: *const CMFormatDescription = std::ptr::null();
    let nal_length_size = AccessUnitSplitter::NAL_LENGTH_SIZE;

    #[expect(unsafe_code)]
    // SAFETY: The pointer and size arrays hold `parameter_sets.len()` entries each and
    // outlive the call, every pointer points at that many readable bytes, and `raw` is
    // a valid out pointer.
    let status = unsafe {
        match codec {
            Codec::H264 => CMVideoFormatDescriptionCreateFromH264ParameterSets(
                None,
                parameter_sets.len(),
                pointers_start,
                NonNull::from(&sizes[0]),
                nal_length_size,
                NonNull::from(&mut raw),
            ),
            Codec::H265 => CMVideoFormatDescriptionCreateFromHEVCParameterSets(
                None,
                parameter_sets.len(),
                pointers_start,
                NonNull::from(&sizes[0]),
                nal_length_size,
                None,
                NonNull::from(&mut raw),
            ),
            Codec::AV1 => {
                return Err(DecodeError::UnsupportedCodec(codec));
            }
        }
    };

    let Some(raw) = NonNull::new(raw.cast_mut()).filter(|_| status == 0) else {
        return Err(DecodeError::VideoToolbox {
            what: "failed to build a format description from the stream's parameter sets",
            status,
        });
    };
    #[expect(unsafe_code)]
    // SAFETY: Both creation functions return an owned reference.
    Ok(unsafe { CFRetained::from_raw(raw) })
}

/// Asks for NV12 frames on an `IOSurface` Metal can wrap.
///
/// Both the video and the full range format are offered so that the decoder keeps the
/// stream's own range instead of converting, `output.rs` reads back which one it picked.
fn destination_image_buffer_attributes() -> CFRetained<CFDictionary> {
    let formats = CFArray::from_retained_objects(&[
        CFNumber::new_i32(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange.cast_signed()),
        CFNumber::new_i32(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange.cast_signed()),
    ]);
    let io_surface_properties = CFDictionary::<CFString, CFType>::empty();

    let keys: [&CFString; 3] = [
        #[expect(unsafe_code)]
        // SAFETY: CoreVideo's own attribute keys, valid for the process's lifetime.
        unsafe {
            kCVPixelBufferPixelFormatTypeKey
        },
        #[expect(unsafe_code)]
        // SAFETY: See above.
        unsafe {
            kCVPixelBufferMetalCompatibilityKey
        },
        #[expect(unsafe_code)]
        // SAFETY: See above.
        unsafe {
            kCVPixelBufferIOSurfacePropertiesKey
        },
    ];
    let values: [&CFType; 3] = [&formats, CFBoolean::new(true), &io_surface_properties];

    CFDictionary::from_slices(&keys, &values)
        .as_opaque()
        .retain()
}

fn create_block_buffer(data: &[u8]) -> Result<CFRetained<CMBlockBuffer>, DecodeError> {
    let mut raw: *mut CMBlockBuffer = std::ptr::null_mut();

    #[expect(unsafe_code)]
    // SAFETY: A null memory block asks CoreMedia to allocate `data.len()` bytes with the
    // default allocator, which `AssureMemoryNow` makes it do right away. `raw` is a valid
    // out pointer.
    let status = unsafe {
        CMBlockBuffer::create_with_memory_block(
            None,
            std::ptr::null_mut(),
            data.len(),
            None,
            std::ptr::null(),
            0,
            data.len(),
            kCMBlockBufferAssureMemoryNowFlag,
            NonNull::from(&mut raw),
        )
    };
    let Some(raw) = NonNull::new(raw).filter(|_| status == 0) else {
        return Err(DecodeError::VideoToolbox {
            what: "failed to allocate a block buffer",
            status,
        });
    };
    #[expect(unsafe_code)]
    // SAFETY: `CMBlockBufferCreateWithMemoryBlock` returns an owned reference.
    let block = unsafe { CFRetained::from_raw(raw) };

    #[expect(unsafe_code)]
    // SAFETY: The block buffer holds `data.len()` bytes starting at offset 0, and the
    // source pointer is readable for that many bytes.
    let status = unsafe {
        CMBlockBuffer::replace_data_bytes(
            NonNull::from(data).cast::<c_void>(),
            &block,
            0,
            data.len(),
        )
    };
    if status != 0 {
        return Err(DecodeError::VideoToolbox {
            what: "failed to fill a block buffer",
            status,
        });
    }

    Ok(block)
}
