//! Decodes an mp4 with AV1 in it to a folder of images.

#![allow(clippy::unwrap_used)]

use std::{
    fs::{File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use indicatif::ProgressBar;
use parking_lot::Mutex;

use re_video::{decode::SyncDecoder, VideoData};

fn main() {
    re_log::setup_logging();

    // frames <video.mp4>
    let args: Vec<_> = std::env::args().collect();
    let Some(video_path) = args.get(1) else {
        println!("Usage: frames <video.mp4>");
        return;
    };
    let output_dir = PathBuf::new().join(Path::new(video_path).with_extension(""));

    println!("Decoding {video_path}");

    let video = std::fs::read(video_path).expect("failed to read video");
    let video = re_video::VideoData::load_mp4(&video).expect("failed to load video");

    println!(
        "{} {}x{}",
        video.gops.len(),
        video.config.coded_width,
        video.config.coded_height
    );

    let mut decoder = create_decoder(&video);

    write_video_frames(&video, decoder.as_mut(), &output_dir);
}

fn create_decoder(video: &VideoData) -> Box<dyn SyncDecoder> {
    match &video.config.stsd.contents {
        re_mp4::StsdBoxContent::Av01(av01_box) => {
            println!("Decoding AV1…");
            Box::new(
                re_video::decode::av1::SyncDav1dDecoder::new()
                    .expect("Failed to start AV1 decoder"),
            )
        }

        re_mp4::StsdBoxContent::Avc1(avc1_box) => {
            println!("Decoding H.264…");
            Box::new(
                re_video::decode::ffmpeg::FfmpegCliH264Decoder::new(
                    avc1_box.clone(),
                    video.timescale,
                )
                .expect("Failed to start H.264 decoder"),
            )
        }

        _ => {
            panic!("Unsupported codec: {}", video.human_readable_codec_string());
        }
    }
}

fn write_video_frames(
    video: &re_video::VideoData,
    decoder: &mut dyn re_video::decode::SyncDecoder,
    output_dir: &PathBuf,
) {
    let progress = ProgressBar::new(video.samples.len() as u64).with_message("Decoding video");
    progress.enable_steady_tick(Duration::from_millis(100));

    let frames = Arc::new(Mutex::new(Vec::new()));
    let on_output = {
        let frames = frames.clone();
        let progress = progress.clone();
        move |frame| {
            progress.inc(1);
            frames.lock().push(frame);
        }
    };

    let start = Instant::now();
    for sample in &video.samples {
        let should_stop = std::sync::atomic::AtomicBool::new(false);
        let chunk = video.get(sample).unwrap();
        decoder.submit_chunk(&should_stop, chunk, &on_output);
    }

    let end = Instant::now();
    progress.finish();

    let frames = frames.lock();

    println!(
        "Decoded {} frames in {:.2}ms",
        frames.len(),
        end.duration_since(start).as_secs_f64() * 1000.0
    );

    println!("Writing frames to {}", output_dir.display());
    std::fs::create_dir_all(output_dir).expect("failed to create output directory");

    let width = num_digits(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        if let Ok(frame) = frame {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(output_dir.join(format!("{i:0width$}.ppm")))
                .expect("failed to oformatpen file");
            match frame.format {
                re_video::PixelFormat::Rgb8Unorm => {
                    write_ppm_rgb24(&mut file, frame.width, frame.height, &frame.data);
                }
                re_video::PixelFormat::Rgba8Unorm => {
                    write_ppm_rgba32(&mut file, frame.width, frame.height, &frame.data);
                }
            }
        }
    }
}

fn num_digits(n: usize) -> usize {
    (n as f64).log10().floor() as usize + 1
}

fn write_ppm_rgb24(file: &mut File, width: u32, height: u32, rgb: &[u8]) {
    assert_eq!(width as usize * height as usize * 3, rgb.len());

    let header = format!("P6\n{width} {height}\n255\n");

    let mut data = Vec::with_capacity(header.len() + width as usize * height as usize * 3);
    data.extend_from_slice(header.as_bytes());

    for rgb in rgb.chunks(3) {
        data.extend_from_slice(&[rgb[0], rgb[1], rgb[2]]);
    }

    file.write_all(&data).expect("failed to write frame data");
}

fn write_ppm_rgba32(file: &mut File, width: u32, height: u32, rgba: &[u8]) {
    assert_eq!(width as usize * height as usize * 4, rgba.len());

    let header = format!("P6\n{width} {height}\n255\n");

    let mut data = Vec::with_capacity(header.len() + width as usize * height as usize * 3);
    data.extend_from_slice(header.as_bytes());

    for rgba in rgba.chunks(4) {
        data.extend_from_slice(&[rgba[0], rgba[1], rgba[2]]);
    }

    file.write_all(&data).expect("failed to write frame data");
}
