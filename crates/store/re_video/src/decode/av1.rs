//! AV1 support.

use std::time::Duration;

use crossbeam::{
    channel::{bounded, unbounded, Receiver, RecvError, Sender, TryRecvError},
    select,
    sync::{Parker, Unparker},
};
use dav1d::{PixelLayout, PlanarImageComponent};
use rav1d::dav1d;

use crate::Time;

use super::{Chunk, Frame, PixelFormat};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An error occrurred during initialization of the decoder.
    ///
    /// No further output will come.
    #[error("Error initializing decoder: {0}")]
    Initialization(dav1d::Error),

    #[error("Decoding error: {0}")]
    Dav1d(dav1d::Error),
}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub struct Decoder {
    _thread: std::thread::JoinHandle<()>,
    unparker: Unparker,

    /// Normal command stream
    command_tx: Sender<Command>,

    /// Fast-track to reset and ignore all command in the normal command stream.
    reset_tx: Sender<()>,
}

impl Decoder {
    pub fn new(on_output: impl Fn(Result<Frame>) + Send + Sync + 'static) -> Self {
        re_tracing::profile_function!();
        let (command_tx, command_rx) = unbounded();
        let (reset_tx, reset_rx) = bounded(1);
        let parker = Parker::new();
        let unparker = parker.unparker().clone();

        let thread = std::thread::Builder::new()
            .name("av1_decoder".into())
            .spawn(move || {
                decoder_thread(&command_rx, &reset_rx, &parker, &on_output);
                re_log::debug!("Closing decoder thread");
            })
            .expect("failed to spawn decoder thread");

        Self {
            _thread: thread,
            unparker,
            command_tx,
            reset_tx,
        }
    }

    /// Submits a single frame for decoding.
    pub fn decode(&self, chunk: Chunk) {
        re_tracing::profile_function!();
        self.command_tx.send(Command::Chunk(chunk)).ok();
        self.unparker.unpark();
    }

    /// Resets the decoder.
    ///
    /// This does not block, all chunks sent to `decode` before this point will be discarded.
    pub fn reset(&self) {
        re_tracing::profile_function!();
        // Ask the decoder to reset its internal state.
        let (tx, rx) = crossbeam::channel::bounded(0);
        self.command_tx.send(Command::Reset(tx)).ok();
        self.reset_tx.send(()).ok();
        self.unparker.unpark();
        _ = rx; // Do not block
    }

    /// Blocks until all pending frames have been decoded.
    pub fn flush(&self) {
        re_tracing::profile_function!();
        // Ask the decoder to notify us once all pending frames have been decoded.
        let (tx, rx) = crossbeam::channel::bounded(0);
        self.command_tx.send(Command::Flush(tx)).ok();
        self.unparker.unpark();
        rx.recv().ok();
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        re_tracing::profile_function!();
        // TODO(emilk): maybe set a "close asap" flag instead?
        self.reset(); // ignore enqueued commands
        self.flush(); // wait for thread to stop
    }
}

enum Command {
    Chunk(Chunk),
    Flush(Sender<()>),
    Reset(Sender<()>),
}

type OutputCallback = dyn Fn(Result<Frame>) + Send + Sync;

fn create_decoder() -> Result<dav1d::Decoder, dav1d::Error> {
    re_tracing::profile_function!();

    let mut settings = dav1d::Settings::new();
    settings.set_strict_std_compliance(false);
    settings.set_max_frame_delay(1);

    dav1d::Decoder::with_settings(&settings)
}

fn decoder_thread(
    command_rx: &Receiver<Command>,
    reset_rx: &Receiver<()>,
    parker: &Parker,
    on_output: &OutputCallback,
) {
    let mut decoder = match create_decoder() {
        Err(err) => {
            on_output(Err(Error::Initialization(err)));
            return;
        }
        Ok(decoder) => decoder,
    };

    loop {
        select! {
            recv(reset_rx) -> reset => {
                match reset {
                    Ok(_) => {
                        // Reset the decoder.
                        re_log::debug!("Received reset");
                        decoder.flush();
                        drain_decoded_frames(&mut decoder);
                        loop {
                            match command_rx.try_recv() {
                                // Discard chunks
                                Ok(Command::Chunk(_)) => {}
                                Ok(Command::Reset(done)) => {
                                    done.try_send(()).ok();
                                    break;
                                }
                                Ok(Command::Flush(done)) => {
                                    done.try_send(()).ok();
                                    // We have not hit a `Reset` yet
                                    break;
                                }
                                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {
                                    break;
                                }
                            }
                        }
                        continue;
                    }
                    Err(RecvError) => {
                        re_log::debug!("RecvError");
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
                        submit_chunk(&mut decoder, chunk, on_output);
                        output_frames(&mut decoder, on_output);
                        continue;
                    }

                    Ok(Command::Flush(done)) => {
                        re_log::debug!("Command::Flush");
                        // All pending frames must have already been decoded, because data sent
                        // through a channel is received in the order it was sent.
                        output_frames(&mut decoder, on_output);
                        done.try_send(()).ok();
                        continue;
                    }

                    Ok(Command::Reset(done)) => {
                        re_log::debug!("Command::Reset");
                        done.try_send(()).ok();
                        continue;
                    }

                    Err(RecvError) => {
                        re_log::debug!("RecvError");
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

fn submit_chunk(decoder: &mut dav1d::Decoder, chunk: Chunk, on_output: &OutputCallback) {
    re_tracing::profile_function!();

    {
        re_tracing::profile_scope!("send_pending_data");
        // always attempt to send pending data first
        // this does nothing if there is no pending data,
        // and is required if a call to `send_data` previously
        // returned `EAGAIN`
        match decoder.send_pending_data() {
            Ok(()) => {}
            Err(err) if err.is_again() => {}
            Err(err) => {
                on_output(Err(Error::Dav1d(err)));
            }
        }
    }

    re_tracing::profile_scope!("send_data");
    match decoder.send_data(
        chunk.data,
        None,
        Some(chunk.timestamp.0),
        Some(chunk.duration.0),
    ) {
        Ok(()) => {}
        Err(err) if err.is_again() => {}
        Err(err) => {
            on_output(Err(Error::Dav1d(err)));
        }
    };
}

fn drain_decoded_frames(decoder: &mut dav1d::Decoder) {
    while let Ok(picture) = decoder.get_picture() {
        _ = picture;
    }
}

fn output_frames(decoder: &mut dav1d::Decoder, on_output: &OutputCallback) {
    loop {
        match decoder.get_picture() {
            Ok(picture) => {
                output_picture(&picture, on_output);
            }
            Err(err) if err.is_again() => {
                // Not enough data yet
                break;
            }
            Err(err) => {
                on_output(Err(Error::Dav1d(err)));
            }
        }
    }
}

fn output_picture(picture: &rav1d::Picture, on_output: &(dyn Fn(Result<Frame>) + Send + Sync)) {
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
