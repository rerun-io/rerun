//! Decodes an mp4 to a folder of images.

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

fn main() {
    // frames <video.mp4>
    let args: Vec<_> = std::env::args().collect();
    let Some(video_path) = args.get(1) else {
        println!("Usage: frames <video.mp4>");
        return;
    };
    let output_dir = PathBuf::new().join(Path::new(video_path).with_extension(""));

    println!("Decoding {video_path}");

    let video_blob = std::fs::read(video_path).expect("failed to read video");
    let video = re_video::VideoData::load_mp4(&video_blob).expect("failed to load video");

    println!(
        "{} {}x{}",
        video.gops.len(),
        video.config.coded_width,
        video.config.coded_height
    );

    let mut decoder = re_video::decode::new_decoder(video_path.to_string(), &video)
        .expect("Failed to create decoder");

    write_video_frames(&video, &video_blob, decoder.as_mut(), &output_dir);
}

fn write_video_frames(
    video: &re_video::VideoData,
    video_blob: &[u8],
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
        let chunk = sample.get(video_blob).unwrap();
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
                .expect("failed to open file");
            write_binary_ppm(&mut file, frame.width, frame.height, &frame.data);
        }
    }
}

fn num_digits(n: usize) -> usize {
    (n as f64).log10().floor() as usize + 1
}

fn write_binary_ppm(file: &mut File, width: u32, height: u32, rgba: &[u8]) {
    let header = format!("P6\n{width} {height}\n255\n");

    let mut data = Vec::with_capacity(header.len() + width as usize * height as usize * 3);
    data.extend_from_slice(header.as_bytes());

    for rgba in rgba.chunks(4) {
        data.extend_from_slice(&[rgba[0], rgba[1], rgba[2]]);
    }

    file.write_all(&data).expect("failed to write frame data");
}
