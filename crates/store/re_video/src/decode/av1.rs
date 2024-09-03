//! AV1 support.

use crate::TimeMs;

use super::PixelFormat;
use super::{Chunk, Frame};
use crossbeam::channel::{unbounded, Receiver, Sender};
use dav1d::PixelLayout;
use dav1d::PlanarImageComponent;

pub struct Decoder {
    thread: std::thread::JoinHandle<()>,
    sample_tx: Sender<Chunk>,
}

impl Decoder {
    pub fn new(on_output: impl Fn(Frame) + Send + Sync + 'static) -> Self {
        let (sample_tx, sample_rx) = unbounded();

        let thread = std::thread::Builder::new()
            .name("av1_decoder".into())
            .spawn(move || decoder_thread(sample_rx, Box::new(on_output)))
            .expect("failed to spawn decoder thread");

        Self { thread, sample_tx }
    }
}

fn decoder_thread(sample_rx: Receiver<Chunk>, on_output: Box<dyn Fn(Frame) + Send + Sync>) {
    let mut settings = dav1d::Settings::new();
    settings.set_n_threads(1);
    settings.set_strict_std_compliance(false);
    settings.set_max_frame_delay(1);
    let mut decoder =
        dav1d::Decoder::with_settings(&settings).expect("failed to initialize dav1d::Decoder");

    while let Ok(chunk) = sample_rx.recv() {
        match decoder.send_data(
            chunk.data,
            None,
            Some(time_to_i64(chunk.timestamp)),
            Some(time_to_i64(chunk.duration)),
        ) {
            Ok(()) => {}
            Err(err) if err.is_again() => {
                // Not enough data yet
                continue;
            }
            Err(err) => {
                // Something went wrong
                panic!("Failed to decode frame: {err}");
            }
        };

        loop {
            match decoder.get_picture() {
                Ok(picture) => {
                    let width = picture.width() as usize;
                    let height = picture.height() as usize;

                    match picture.pixel_layout() {
                        // Monochrome
                        PixelLayout::I400 => todo!(),
                        // 4:2:0 planar
                        PixelLayout::I420 => todo!(),
                        // 4:2:2 planar
                        PixelLayout::I422 => todo!(),
                        // 4:4:4 planar
                        PixelLayout::I444 => todo!(),
                    }
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
}

/// We need to convert between `TimeMs` and `i64` because `dav1d` uses `i64` for timestamps.
fn time_to_i64(time: TimeMs) -> i64 {
    // multiply by 1000 to lose less precision
    (time.as_f64() * 1000.0) as i64
}

fn i64_to_time(i64: i64) -> TimeMs {
    TimeMs::new(i64 as f64 / 1000.0)
}
