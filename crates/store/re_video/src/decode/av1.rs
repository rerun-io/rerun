//! AV1 support.

use std::sync::atomic::{AtomicBool, Ordering};

use dav1d::{PixelLayout, PlanarImageComponent};

use crate::Time;

use super::{Chunk, Error, Frame, OutputCallback, PixelFormat, Result, SyncDecoder};

pub struct SyncDav1dDecoder {
    decoder: dav1d::Decoder,
}

impl SyncDav1dDecoder {
    pub fn new() -> Result<Self> {
        re_tracing::profile_function!();

        // TODO(#7671): enable this check again when the `nasm` feature actually does something
        if false && !cfg!(feature = "nasm") {
            re_log::warn_once!(
                "NOTE: native AV1 video decoder is running extra slowly. \
                Speed it up by compiling Rerun with the `nasm` feature enabled. \
                You'll need to also install nasm: https://nasm.us/"
            );
        }

        // See https://videolan.videolan.me/dav1d/structDav1dSettings.html for settings docs
        let mut settings = dav1d::Settings::new();

        // Prioritize delivering video frames, not error messages.
        settings.set_strict_std_compliance(false);

        // Set to 1 for low-latency decoding.
        settings.set_max_frame_delay(1);

        let decoder = dav1d::Decoder::with_settings(&settings)?;

        Ok(Self { decoder })
    }
}

impl SyncDecoder for SyncDav1dDecoder {
    fn submit_chunk(&mut self, should_stop: &AtomicBool, chunk: Chunk, on_output: &OutputCallback) {
        re_tracing::profile_function!();
        submit_chunk(&mut self.decoder, chunk, on_output);
        output_frames(should_stop, &mut self.decoder, on_output);
    }

    /// Clear and reset everything
    fn reset(&mut self) {
        re_tracing::profile_function!();

        self.decoder.flush();

        debug_assert!(matches!(self.decoder.get_picture(), Err(dav1d::Error::Again)),
            "There should be no pending pictures, since we output them directly after submitting a chunk.");
    }
}

fn submit_chunk(decoder: &mut dav1d::Decoder, chunk: Chunk, on_output: &OutputCallback) {
    re_tracing::profile_function!();
    econtext::econtext_function_data!(format!("chunk timestamp: {:?}", chunk.timestamp));

    re_tracing::profile_scope!("send_data");
    match decoder.send_data(
        chunk.data,
        None,
        Some(chunk.timestamp.0),
        Some(chunk.duration.0),
    ) {
        Ok(()) => {}
        Err(err) => {
            debug_assert!(err != dav1d::Error::Again, "Bug in AV1 decoder: send_data returned `Error::Again`. This shouldn't happen, since we process all images in a chunk right away");
            on_output(Err(Error::Dav1d(err)));
        }
    };
}

/// Returns the number of new frames.
fn output_frames(
    should_stop: &AtomicBool,
    decoder: &mut dav1d::Decoder,
    on_output: &OutputCallback,
) -> usize {
    re_tracing::profile_function!();
    let mut count = 0;
    while !should_stop.load(Ordering::SeqCst) {
        let picture = {
            econtext::econtext!("get_picture");
            decoder.get_picture()
        };
        match picture {
            Ok(picture) => {
                output_picture(&picture, on_output);
                count += 1;
            }
            Err(dav1d::Error::Again) => {
                // We need to submit more chunks to get more pictures
                break;
            }
            Err(err) => {
                on_output(Err(Error::Dav1d(err)));
            }
        }
    }
    count
}

fn output_picture(picture: &dav1d::Picture, on_output: &(dyn Fn(Result<Frame>) + Send + Sync)) {
    // TODO(jan): support other parameters?
    // What do these even do:
    // - matrix_coefficients
    // - color_range
    // - color_primaries
    // - transfer_characteristics

    let frame = Frame {
        data: match picture.pixel_layout() {
            PixelLayout::I400 => i400_to_rgba(picture),
            PixelLayout::I420 => i420_to_rgba(picture),
            PixelLayout::I422 => i422_to_rgba(picture),
            PixelLayout::I444 => i444_to_rgba(picture),
        },
        width: picture.width(),
        height: picture.height(),
        format: PixelFormat::Rgba8Unorm,
        timestamp: Time(picture.timestamp().unwrap_or(0)),
        duration: Time(picture.duration()),
    };
    on_output(Ok(frame));
}

fn rgba_from_yuv(y: u8, u: u8, v: u8) -> [u8; 4] {
    let (y, u, v) = (f32::from(y), f32::from(u), f32::from(v));

    // Adjust for color range
    let y = (y - 16.0) / 219.0;
    let u = (u - 128.0) / 224.0;
    let v = (v - 128.0) / 224.0;

    // BT.601 coefficients
    let r = y + 1.402 * v;
    let g = y - 0.344136 * u - 0.714136 * v;
    let b = y + 1.772 * u;

    [
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
        255, // Alpha channel, fully opaque
    ]
}

fn i400_to_rgba(picture: &dav1d::Picture) -> Vec<u8> {
    re_tracing::profile_function!();

    let width = picture.width() as usize;
    let height = picture.height() as usize;
    let y_plane = picture.plane(PlanarImageComponent::Y);
    let y_stride = picture.stride(PlanarImageComponent::Y) as usize;

    let mut rgba = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let y_value = y_plane[y * y_stride + x];
            let rgba_pixel = rgba_from_yuv(y_value, 128, 128);

            let offset = y * width * 4 + x * 4;
            rgba[offset] = rgba_pixel[0];
            rgba[offset + 1] = rgba_pixel[1];
            rgba[offset + 2] = rgba_pixel[2];
            rgba[offset + 3] = rgba_pixel[3];
        }
    }

    rgba
}

fn i420_to_rgba(picture: &dav1d::Picture) -> Vec<u8> {
    re_tracing::profile_function!();

    let width = picture.width() as usize;
    let height = picture.height() as usize;
    let y_plane = picture.plane(PlanarImageComponent::Y);
    let u_plane = picture.plane(PlanarImageComponent::U);
    let v_plane = picture.plane(PlanarImageComponent::V);
    let y_stride = picture.stride(PlanarImageComponent::Y) as usize;
    let uv_stride = picture.stride(PlanarImageComponent::U) as usize;

    let mut rgba = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let y_value = y_plane[y * y_stride + x];
            let u_value = u_plane[(y / 2) * uv_stride + (x / 2)];
            let v_value = v_plane[(y / 2) * uv_stride + (x / 2)];
            let rgba_pixel = rgba_from_yuv(y_value, u_value, v_value);

            let offset = y * width * 4 + x * 4;
            rgba[offset] = rgba_pixel[0];
            rgba[offset + 1] = rgba_pixel[1];
            rgba[offset + 2] = rgba_pixel[2];
            rgba[offset + 3] = rgba_pixel[3];
        }
    }

    rgba
}

fn i422_to_rgba(picture: &dav1d::Picture) -> Vec<u8> {
    re_tracing::profile_function!();

    let width = picture.width() as usize;
    let height = picture.height() as usize;
    let y_plane = picture.plane(PlanarImageComponent::Y);
    let u_plane = picture.plane(PlanarImageComponent::U);
    let v_plane = picture.plane(PlanarImageComponent::V);
    let y_stride = picture.stride(PlanarImageComponent::Y) as usize;
    let uv_stride = picture.stride(PlanarImageComponent::U) as usize;

    let mut rgba = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let y_value = y_plane[y * y_stride + x];
            let u_value = u_plane[y * uv_stride + (x / 2)];
            let v_value = v_plane[y * uv_stride + (x / 2)];
            let rgba_pixel = rgba_from_yuv(y_value, u_value, v_value);

            let offset = y * width * 4 + x * 4;
            rgba[offset] = rgba_pixel[0];
            rgba[offset + 1] = rgba_pixel[1];
            rgba[offset + 2] = rgba_pixel[2];
            rgba[offset + 3] = rgba_pixel[3];
        }
    }

    rgba
}

fn i444_to_rgba(picture: &dav1d::Picture) -> Vec<u8> {
    re_tracing::profile_function!();

    let width = picture.width() as usize;
    let height = picture.height() as usize;
    let y_plane = picture.plane(PlanarImageComponent::Y);
    let u_plane = picture.plane(PlanarImageComponent::U);
    let v_plane = picture.plane(PlanarImageComponent::V);
    let y_stride = picture.stride(PlanarImageComponent::Y) as usize;
    let u_stride = picture.stride(PlanarImageComponent::U) as usize;
    let v_stride = picture.stride(PlanarImageComponent::V) as usize;

    let mut rgba = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let y_value = y_plane[y * y_stride + x];
            let u_value = u_plane[y * u_stride + x];
            let v_value = v_plane[y * v_stride + x];
            let rgba_pixel = rgba_from_yuv(y_value, u_value, v_value);

            let offset = y * width * 4 + x * 4;
            rgba[offset] = rgba_pixel[0];
            rgba[offset + 1] = rgba_pixel[1];
            rgba[offset + 2] = rgba_pixel[2];
            rgba[offset + 3] = rgba_pixel[3];
        }
    }

    rgba
}
