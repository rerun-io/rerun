//! AV1 support.

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use crossbeam::channel::{unbounded, Receiver, Sender};
use dav1d::{PixelLayout, PlanarImageComponent};

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

enum Command {
    Chunk(Chunk),
    Flush { on_done: Sender<()> },
    Reset,
    Stop,
}

#[derive(Clone)]
struct Comms {
    /// Set when it is time to die
    should_stop: Arc<AtomicBool>,

    /// Incremented on each call to [`Decoder::reset`].
    /// Decremented each time the decoder thread receives [`Command::Reset`].
    num_outstanding_resets: Arc<AtomicU64>,
}

impl Default for Comms {
    fn default() -> Self {
        Self {
            should_stop: Arc::new(AtomicBool::new(false)),
            num_outstanding_resets: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// AV1 software decoder.
pub struct Decoder {
    /// Where the decoding happens
    _thread: std::thread::JoinHandle<()>,

    /// Commands sent to the decoder thread.
    command_tx: Sender<Command>,

    /// Instant communication to the decoder thread (circumventing the command queue).
    comms: Comms,
}

impl Decoder {
    pub fn new(
        debug_name: String,
        on_output: impl Fn(Result<Frame>) + Send + Sync + 'static,
    ) -> Self {
        re_tracing::profile_function!();
        let (command_tx, command_rx) = unbounded();
        let comms = Comms::default();

        let thread = std::thread::Builder::new()
            .name("av1_decoder".into())
            .spawn({
                let comms = comms.clone();
                move || {
                    econtext::econtext_data!("Video", debug_name.clone());
                    decoder_thread(&comms, &command_rx, &on_output);
                    re_log::debug!("Closing decoder thread for {debug_name}");
                }
            })
            .expect("failed to spawn decoder thread");

        Self {
            _thread: thread,
            command_tx,
            comms,
        }
    }

    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    pub fn decode(&mut self, chunk: Chunk) {
        re_tracing::profile_function!();
        self.command_tx.send(Command::Chunk(chunk)).ok();
    }

    /// Resets the decoder.
    ///
    /// This does not block, all chunks sent to `decode` before this point will be discarded.
    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    pub fn reset(&mut self) {
        re_tracing::profile_function!();
        self.comms
            .num_outstanding_resets
            .fetch_add(1, Ordering::SeqCst);
        self.command_tx.send(Command::Reset).ok();
    }

    /// Blocks until all pending frames have been decoded.
    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    pub fn flush(&mut self) {
        re_tracing::profile_function!();
        let (tx, rx) = crossbeam::channel::bounded(0);
        self.command_tx.send(Command::Flush { on_done: tx }).ok();
        rx.recv().ok();
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        re_tracing::profile_function!();
        self.comms.should_stop.store(true, Ordering::SeqCst);
        self.command_tx.send(Command::Stop).ok();
        // NOTE: we don't block here. The decoder thread will finish soon enough.
    }
}

type OutputCallback = dyn Fn(Result<Frame>) + Send + Sync;

fn create_decoder() -> Result<dav1d::Decoder, dav1d::Error> {
    re_tracing::profile_function!();

    let mut settings = dav1d::Settings::new();
    settings.set_strict_std_compliance(false);
    settings.set_max_frame_delay(1);

    dav1d::Decoder::with_settings(&settings)
}

fn decoder_thread(comms: &Comms, command_rx: &Receiver<Command>, on_output: &OutputCallback) {
    let mut decoder = match create_decoder() {
        Err(err) => {
            on_output(Err(Error::Initialization(err)));
            return;
        }
        Ok(decoder) => decoder,
    };

    while let Ok(command) = command_rx.recv() {
        if comms.should_stop.load(Ordering::SeqCst) {
            re_log::debug!("Should stop");
            return;
        }

        // If we're waiting for a reset we should ignore all other commands until we receive it.
        let has_outstanding_reset = 0 < comms.num_outstanding_resets.load(Ordering::SeqCst);

        match command {
            Command::Chunk(chunk) => {
                if !has_outstanding_reset {
                    submit_chunk(&mut decoder, chunk, on_output);
                    output_frames(&comms.should_stop, &mut decoder, on_output);
                }
            }
            Command::Flush { on_done } => {
                if !has_outstanding_reset {
                    // TODO(emilk): can there really be outstanding frames here?
                    output_frames(&comms.should_stop, &mut decoder, on_output);
                }
                on_done.send(()).ok();
            }
            Command::Reset => {
                decoder.flush();
                drain_decoded_frames(&mut decoder);
                comms.num_outstanding_resets.fetch_sub(1, Ordering::SeqCst);
            }
            Command::Stop => {
                re_log::debug!("Stop");
                return;
            }
        }
    }

    re_log::debug!("Disconnected");
}

fn submit_chunk(decoder: &mut dav1d::Decoder, chunk: Chunk, on_output: &OutputCallback) {
    re_tracing::profile_function!();
    econtext::econtext_function_data!(format!("chunk timestamp: {:?}", chunk.timestamp));

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
    re_tracing::profile_function!();
    while let Ok(picture) = decoder.get_picture() {
        _ = picture;
    }
}

fn output_frames(
    should_stop: &AtomicBool,
    decoder: &mut dav1d::Decoder,
    on_output: &OutputCallback,
) {
    re_tracing::profile_function!();
    while !should_stop.load(Ordering::SeqCst) {
        match {
            econtext::econtext!("get_picture");
            decoder.get_picture()
        } {
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
