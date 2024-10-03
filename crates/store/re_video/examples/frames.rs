#![allow(clippy::unwrap_used)]

use indicatif::ProgressBar;
use parking_lot::Mutex;
use re_video::demux::mp4::load_mp4;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

fn main() {
    // frames <video.mp4>
    let args: Vec<_> = std::env::args().collect();
    let Some(video_path) = args.get(1) else {
        println!("Usage: frames <video.mp4>");
        return;
    };
    let output_dir = PathBuf::new().join(Path::new(video_path).with_extension(""));

    println!("Decoding {video_path}");

    let video = std::fs::read(video_path).expect("failed to read video");
    let video = load_mp4(&video).expect("failed to load video");

    println!(
        "{} {}x{}",
        video.segments.len(),
        video.config.coded_width,
        video.config.coded_height
    );

    let progress = ProgressBar::new(video.samples.len() as u64).with_message("Decoding video");
    progress.enable_steady_tick(Duration::from_millis(100));

    let frames = Arc::new(Mutex::new(Vec::new()));
    let decoder = re_video::decode::av1::Decoder::new({
        let frames = frames.clone();
        let progress = progress.clone();
        move |frame| {
            progress.inc(1);
            frames.lock().push(frame);
        }
    });

    let start = Instant::now();
    for sample in &video.samples {
        decoder.decode(video.get(sample).unwrap());
    }

    drop(decoder);
    let end = Instant::now();
    progress.finish();

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
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(output_dir.join(format!("{i:0width$}.ppm")))
            .expect("failed to open file");
        write_binary_ppm(&mut file, frame.width, frame.height, &frame.data);
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