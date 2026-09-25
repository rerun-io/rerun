//! macOS playback through an `AudioQueue` from `AudioToolbox`.
//!
//! We do this ourselves instead of using `tinyaudio` (as on the other platforms) because
//! `tinyaudio` depends on `coreaudio-sys`, which runs bindgen on the macOS SDK headers at
//! build time. That needs libclang and fails in our cross-compiled macOS CI builds.
//! `objc2-audio-toolbox` ships pregenerated bindings instead.
//!
//! TODO(mrDIMAS/tinyaudio#26): remove this module once `tinyaudio` uses `objc2-audio-toolbox`.

#![expect(unsafe_code, reason = "FFI into AudioToolbox")]

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_audio_toolbox::{
    AudioQueueAllocateBuffer, AudioQueueBufferRef, AudioQueueDispose, AudioQueueEnqueueBuffer,
    AudioQueueNewOutput, AudioQueueRef, AudioQueueStart, AudioQueueStop,
};
use objc2_core_audio_types::{
    AudioStreamBasicDescription, kAudioFormatLinearPCM, kLinearPCMFormatFlagIsFloat,
    kLinearPCMFormatFlagIsPacked,
};

use super::{FailureCallback, FillCallback, OutputDeviceParameters, OutputError};

/// Buffers cycled through the queue: one playing, the others filled and waiting.
const NUM_BUFFERS: usize = 3;

/// State the queue's callback reaches through its user-data pointer.
struct CallbackContext {
    params: OutputDeviceParameters,
    fill: FillCallback,

    /// Taken on the first failure, so it is reported only once.
    on_failure: Option<FailureCallback>,
}

pub struct Device {
    /// Null until the queue is created.
    queue: AudioQueueRef,

    /// Owned, from [`Box::leak`]. The queue's callback mutates it through this pointer,
    /// so it must outlive the queue, which [`Drop`] ensures by disposing of the queue first.
    context: NonNull<CallbackContext>,
}

impl Drop for Device {
    fn drop(&mut self) {
        if !self.queue.is_null() {
            // SAFETY: the queue is valid. Stopping and disposing immediately is synchronous,
            // so no callback runs after this.
            unsafe {
                AudioQueueStop(self.queue, true);
                AudioQueueDispose(self.queue, true);
            }
        }
        // SAFETY: the pointer came from `Box::leak`, and the queue that used it is gone.
        drop(unsafe { Box::from_raw(self.context.as_ptr()) });
    }
}

impl Device {
    /// Opens an output queue on the default device and starts calling `fill` for samples.
    ///
    /// The queue runs its callbacks on its own internal thread.
    pub fn new(
        params: OutputDeviceParameters,
        fill: FillCallback,
        on_failure: FailureCallback,
    ) -> Result<Self, OutputError> {
        re_tracing::profile_function!();

        let bytes_per_frame = (params.num_channels * size_of::<f32>()) as u32;
        let format = AudioStreamBasicDescription {
            mSampleRate: params.sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kLinearPCMFormatFlagIsFloat | kLinearPCMFormatFlagIsPacked,
            mBytesPerPacket: bytes_per_frame,
            mFramesPerPacket: 1,
            mBytesPerFrame: bytes_per_frame,
            mChannelsPerFrame: params.num_channels as u32,
            mBitsPerChannel: 32,
            mReserved: 0,
        };

        // Dropping `device` cleans up on every error path below.
        let mut device = Self {
            queue: std::ptr::null_mut(),
            context: NonNull::from(Box::leak(Box::new(CallbackContext {
                params,
                fill,
                on_failure: Some(on_failure),
            }))),
        };

        // SAFETY: `format` and `device.queue` are valid for the call. The user data is the
        // context, which outlives the queue (see `Device`).
        check("AudioQueueNewOutput", unsafe {
            AudioQueueNewOutput(
                NonNull::from(&format),
                Some(fill_buffer),
                device.context.as_ptr().cast(),
                None,
                None,
                0,
                NonNull::from(&mut device.queue),
            )
        })?;
        if device.queue.is_null() {
            return Err(OutputError::Device(
                "AudioQueueNewOutput succeeded but returned no queue".to_owned(),
            ));
        }

        let buffer_bytes = params.frames_per_callback as u32 * bytes_per_frame;
        for _ in 0..NUM_BUFFERS {
            let mut buffer: AudioQueueBufferRef = std::ptr::null_mut();
            // SAFETY: the queue is valid and `buffer` is valid for the call.
            check("AudioQueueAllocateBuffer", unsafe {
                AudioQueueAllocateBuffer(device.queue, buffer_bytes, NonNull::from(&mut buffer))
            })?;

            // Prime the queue with silence, so playback starts without waiting on `fill`.
            // SAFETY: the queue just allocated `buffer` with `buffer_bytes` of capacity.
            unsafe {
                let buffer = &mut *buffer;
                std::ptr::write_bytes(
                    buffer.mAudioData.as_ptr().cast::<u8>(),
                    0,
                    buffer_bytes as usize,
                );
                buffer.mAudioDataByteSize = buffer_bytes;
            }
            // SAFETY: the buffer belongs to this queue and is not enqueued yet.
            check("AudioQueueEnqueueBuffer", unsafe {
                AudioQueueEnqueueBuffer(device.queue, buffer, 0, std::ptr::null())
            })?;
        }

        // SAFETY: the queue is valid; a null start time means "as soon as possible".
        check("AudioQueueStart", unsafe {
            AudioQueueStart(device.queue, std::ptr::null())
        })?;

        Ok(device)
    }
}

/// Called by the queue on its own thread whenever a buffer finished playing.
unsafe extern "C-unwind" fn fill_buffer(
    user_data: *mut c_void,
    queue: AudioQueueRef,
    buffer: AudioQueueBufferRef,
) {
    // SAFETY: `user_data` is the boxed context, which outlives the queue, and the queue calls
    // back on one thread at a time. `buffer` belongs to the queue and is not playing.
    let (context, buffer) = unsafe { (&mut *user_data.cast::<CallbackContext>(), &mut *buffer) };

    let num_samples = context.params.frames_per_callback * context.params.num_channels;
    // SAFETY: every buffer was allocated with room for exactly `num_samples` samples,
    // and the queue aligns buffer memory for any sample type.
    let samples = unsafe {
        std::slice::from_raw_parts_mut(buffer.mAudioData.as_ptr().cast::<f32>(), num_samples)
    };

    (context.fill)(&context.params, samples);
    buffer.mAudioDataByteSize = size_of_val(samples) as u32;

    // SAFETY: the buffer belongs to this queue and was just filled.
    let status = unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null()) };
    if let Err(err) = check("AudioQueueEnqueueBuffer", status)
        && let Some(on_failure) = context.on_failure.take()
    {
        on_failure(err);
    }
}

fn check(what: &str, status: i32) -> Result<(), OutputError> {
    if status == 0 {
        Ok(())
    } else {
        Err(OutputError::Device(format!("{what} failed with OSStatus {status}")))
    }
}
