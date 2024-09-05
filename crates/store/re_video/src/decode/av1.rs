//! AV1 support.

use super::PixelFormat;
use super::{Chunk, Frame};
use crate::TimeMs;
use crossbeam::channel::bounded;
use crossbeam::channel::RecvError;
use crossbeam::channel::{unbounded, Receiver, Sender};
use crossbeam::select;
use crossbeam::sync::Parker;
use crossbeam::sync::Unparker;
use dav1d::PixelLayout;
use dav1d::PlanarImageComponent;
use std::time::Duration;

pub struct Decoder {
    _thread: std::thread::JoinHandle<()>,
    unparker: Unparker,
    command_tx: Sender<Command>,
    flush_rx: Receiver<()>,
    reset_tx: Sender<()>,
}

impl Decoder {
    pub fn new(on_output: impl Fn(Frame) + Send + Sync + 'static) -> Self {
        let (command_tx, command_rx) = unbounded();
        let (flush_tx, flush_rx) = bounded(1);
        let (reset_tx, reset_rx) = bounded(1);
        let parker = Parker::new();
        let unparker = parker.unparker().clone();

        let thread = std::thread::Builder::new()
            .name("av1_decoder".into())
            .spawn(move || decoder_thread(&command_rx, &reset_rx, &flush_tx, &parker, &on_output))
            .expect("failed to spawn decoder thread");

        Self {
            _thread: thread,
            unparker,
            command_tx,
            reset_tx,
            flush_rx,
        }
    }

    /// Submits a single frame for decoding.
    pub fn decode(&self, chunk: Chunk) {
        self.command_tx.send(Command::Chunk(chunk)).ok();
        self.unparker.unpark();
    }

    /// Resets the decoder.
    ///
    /// This does not block, all chunks sent to `decode` before this point will be discarded.
    pub fn reset(&self) {
        // Ask the decoder to reset its internal state.
        self.reset_tx.send(()).ok();
        self.unparker.unpark();
    }

    /// Blocks until all pending frames have been decoded.
    pub fn flush(&self) {
        // Ask the decoder to notify us once all pending frames have been decoded.
        self.command_tx.send(Command::Flush).ok();
        self.unparker.unpark();
        self.flush_rx.recv().ok();
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.flush();
    }
}

enum Command {
    Chunk(Chunk),
    Flush,
}

type OutputCallback = dyn Fn(Frame) + Send + Sync;

fn decoder_thread(
    command_rx: &Receiver<Command>,
    reset_rx: &Receiver<()>,
    flush_tx: &Sender<()>,
    parker: &Parker,
    on_output: &OutputCallback,
) {
    let mut settings = dav1d::Settings::new();
    settings.set_strict_std_compliance(false);
    settings.set_max_frame_delay(1);

    let mut decoder =
        dav1d::Decoder::with_settings(&settings).expect("failed to initialize dav1d::Decoder");

    loop {
        select! {
            recv(reset_rx) -> reset => {
                match reset {
                    Ok(_) => {
                        // Reset the decoder.
                        decoder.flush();
                        drain_decoded_frames(&mut decoder);
                        continue;
                    }
                    Err(RecvError) => {
                        // Channel disconnected, this only happens if the decoder is dropped.
                        decoder.flush();
                        output_frames(&mut decoder, on_output);
                        break;
                    }
                }
            }

            recv(command_rx) -> command => {
                match command {
                    Ok(Command::Chunk(chunk)) => {
                        submit_chunk(&mut decoder, chunk);
                        output_frames(&mut decoder, on_output);
                        continue;
                    }

                    Ok(Command::Flush) => {
                        // All pending frames must have already been decoded, because data sent
                        // through a channel is received in the order it was sent.
                        output_frames(&mut decoder, on_output);
                        flush_tx.try_send(()).ok();
                        continue;
                    }

                    Err(RecvError) => {
                        // Channel disconnected, this only happens if the decoder is dropped.
                        decoder.flush();
                        output_frames(&mut decoder, on_output);
                        break;
                    }
                }
            }

            default => {
                // No samples left in the queue
                parker.park_timeout(Duration::from_millis(100));
                continue;
            }
        }
    }
}

fn submit_chunk(decoder: &mut dav1d::Decoder, chunk: Chunk) {
    // always attempt to send pending data first
    // this does nothing if there is no pending data,
    // and is required if a call to `send_data` previously
    // returned `EAGAIN`
    match decoder.send_pending_data() {
        Ok(()) => {}
        Err(err) if err.is_again() => {}
        Err(err) => {
            // Something went wrong
            panic!("Failed to decode frame: {err}");
        }
    }

    match decoder.send_data(
        chunk.data,
        None,
        Some(time_to_i64(chunk.timestamp)),
        Some(time_to_i64(chunk.duration)),
    ) {
        Ok(()) => {}
        Err(err) if err.is_again() => {}
        Err(err) => {
            // Something went wrong
            panic!("Failed to decode frame: {err}");
        }
    };
}

fn drain_decoded_frames(decoder: &mut dav1d::Decoder) {
    while let Ok(picture) = decoder.get_picture() {
        let _ = picture;
    }
}

fn output_frames(decoder: &mut dav1d::Decoder, on_output: &OutputCallback) {
    loop {
        match decoder.get_picture() {
            Ok(picture) => {
                let data = match picture.pixel_layout() {
                    PixelLayout::I400 => i400_to_rgba(&picture),
                    PixelLayout::I420 => i420_to_rgba(&picture),
                    PixelLayout::I422 => i422_to_rgba(&picture),
                    PixelLayout::I444 => i444_to_rgba(&picture),
                };
                let width = picture.width();
                let height = picture.height();
                let timestamp = i64_to_time(picture.timestamp().unwrap_or(0));
                let duration = i64_to_time(picture.duration());

                on_output(Frame {
                    data,
                    width,
                    height,
                    format: PixelFormat::Rgba8Unorm,
                    timestamp,
                    duration,
                });
            }
            Err(err) if err.is_again() => {
                // Not enough data yet
                break;
            }
            Err(err) => {
                panic!("Failed to decode frame: {err}");
            }
        }
    }
}

// TODO(jan): support other parameters?
// What do these even do:
// - matrix_coefficients
// - color_range
// - color_primaries
// - transfer_characteristics

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

// We need to convert between `TimeMs` and `i64` because `dav1d` uses `i64` for timestamps.
fn time_to_i64(time: TimeMs) -> i64 {
    // multiply by 1000 to lose less precision
    (time.as_f64() * 1000.0) as i64
}

fn i64_to_time(i64: i64) -> TimeMs {
    TimeMs::new(i64 as f64 / 1000.0)
}
