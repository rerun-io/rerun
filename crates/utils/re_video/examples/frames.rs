//! Decodes an mp4 to a folder of images.

#![expect(clippy::unwrap_used)]

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use indicatif::ProgressBar;
use parking_lot::Mutex;

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

    let video_blob = std::fs::read(video_path).expect("failed to read video");
    let source_id = re_tuid::Tuid::new();
    let video = re_video::VideoDataDescription::load_mp4(&video_blob, video_path, source_id)
        .expect("failed to load video");

    println!(
        "{} {}x{}",
        video.keyframe_indices.len(),
        video
            .encoding_details
            .as_ref()
            .map_or(0, |c| c.coded_dimensions[0]),
        video
            .encoding_details
            .as_ref()
            .map_or(0, |c| c.coded_dimensions[1])
    );

    let progress = Arc::new(
        ProgressBar::new(video.samples.num_elements() as u64).with_message("Decoding video"),
    );
    progress.enable_steady_tick(Duration::from_millis(100));

    let frames = Arc::new(Mutex::new(Vec::new()));
    let (output_sender, output_receiver) = crossbeam::channel::unbounded();

    let output_thread = std::thread::Builder::new()
        .name("output".to_owned())
        .spawn({
            let progress = progress.clone();
            let frames = frames.clone();
            let num_frames_expected = video.samples.num_elements() as u64;
            move || {
                while let Ok(frame) = output_receiver.recv() {
                    progress.inc(1);
                    frames.lock().push(frame);

                    if progress.position() == num_frames_expected {
                        progress.finish();
                        break;
                    }
                }
            }
        })
        .expect("Failed to start output thread.");

    let mut decoder = re_video::new_decoder(
        video_path,
        &video,
        &re_video::DecodeSettings::default(),
        output_sender,
    )
    .expect("Failed to create decoder");

    let start = Instant::now();
    for (sample_idx, sample) in video.samples.iter_indexed() {
        let Some(sample) = sample.sample() else {
            continue;
        };

        let chunk = sample.get(&|_| &video_blob, sample_idx).unwrap();
        decoder.submit_chunk(chunk).expect("Failed to submit chunk");
    }
    decoder.end_of_video().expect("Failed to end of video");

    output_thread.join().expect("Failed to join output thread");

    let end = Instant::now();
    let frames = frames.lock();

    println!(
        "Decoded {} frames in {:.2}ms",
        frames.len(),
        end.duration_since(start).as_secs_f64() * 1000.0
    );

    println!("Writing frames to {}", output_dir.display());
    std::fs::create_dir_all(&output_dir).expect("failed to create output directory");

    let width = num_digits(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        if let Ok(frame) = frame {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(output_dir.join(format!("{i:0width$}.ppm")))
                .expect("failed to open file");

            let frame = &frame.content;
            match frame.format {
                re_video::PixelFormat::Rgb8Unorm => {
                    write_ppm_rgb24(&mut file, frame.width, frame.height, &frame.data);
                }
                re_video::PixelFormat::Rgba8Unorm => {
                    write_ppm_rgba32(&mut file, frame.width, frame.height, &frame.data);
                }
                re_video::PixelFormat::Yuv { .. } => {
                    re_log::error_once!("YUV frame writing is not supported");
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
