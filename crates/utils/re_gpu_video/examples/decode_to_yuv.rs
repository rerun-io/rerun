//! Decodes an H.264 elementary stream (annex-b) on the GPU and writes the frames as
//! raw planar yuv420p, for PSNR comparison against ffmpeg's software decoder:
//!
//! ```sh
//! cargo run -p re_gpu_video --example decode_to_yuv -- tests/assets/ipb.h264 /tmp/gpu.yuv
//! ffmpeg -i tests/assets/ipb.h264 -f rawvideo -pix_fmt yuv420p /tmp/ref.yuv
//! ffmpeg -f rawvideo -video_size WxH -pix_fmt yuv420p -i /tmp/gpu.yuv \
//!        -f rawvideo -video_size WxH -pix_fmt yuv420p -i /tmp/ref.yuv \
//!        -lavfi psnr -f null -
//! ```
//!
//! Per-frame PSNR above ~40 dB means the DPB and reference-list handling is correct.

use std::io::Write as _;

use re_gpu_video::{CpuFrame, VideoDeviceSetup};

fn main() {
    re_log::setup_logging();

    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("Usage: decode_to_yuv <input.h264> <output.yuv>");
        std::process::exit(1);
    };

    let data = std::fs::read(&input).expect("failed to read the input file");

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    let Some((adapter, mut setup)) = adapters
        .iter()
        .find_map(|adapter| VideoDeviceSetup::request(adapter).map(|setup| (adapter, setup)))
    else {
        eprintln!("No adapter with H.264 decode support found.");
        std::process::exit(1);
    };
    println!(
        "Decoding on {} using {:#?}",
        adapter.get_info().name,
        setup.capabilities()
    );

    let descriptor = wgpu::DeviceDescriptor {
        label: Some("decode_to_yuv"),
        required_features: adapter
            .features()
            .intersection(wgpu::Features::TEXTURE_FORMAT_NV12),
        ..Default::default()
    };
    let (device, _queue) = if setup.needs_hal_device_creation() {
        #[expect(unsafe_code)]
        // SAFETY: Mirrors what `re_renderer::device::create_device` does:
        // the hal device is created from this adapter and handed straight to wgpu.
        unsafe {
            let hal_adapter = adapter
                .as_hal::<wgpu::hal::api::Vulkan>()
                .expect("probed adapter must be a Vulkan adapter");
            let open_device = hal_adapter
                .open_with_callback(
                    descriptor.required_features,
                    &descriptor.required_limits,
                    &descriptor.memory_hints,
                    Some(setup.create_device_callback()),
                )
                .expect("hal device creation failed");
            adapter
                .create_device_from_hal(open_device, &descriptor)
                .expect("wgpu device creation failed")
        }
    } else {
        pollster::block_on(adapter.request_device(&descriptor))
            .expect("wgpu device creation failed")
    };

    let context = setup
        .into_context(&device)
        .expect("video context creation failed");
    let mut decoder = context
        .create_h264_cpu_decoder()
        .expect("decoder creation failed");

    // The parser splits frames on its own, the whole stream can go in as one push.
    let frames = decoder.push_access_unit(&data).expect("decoding failed");

    let mut file = std::io::BufWriter::new(
        std::fs::File::create(&output).expect("failed to create the output file"),
    );

    // Frames arrive in decode order. Presentation order sorts by picture order count,
    // within the groups delimited by IDR frames (an IDR resets the count to zero and
    // no frame is presented across it in the other order).
    let mut pending: Vec<CpuFrame> = Vec::new();
    let mut written = 0;
    let mut size = None;
    let mut flush = |pending: &mut Vec<CpuFrame>, file: &mut dyn std::io::Write| {
        pending.sort_by_key(|frame| frame.poc);
        for frame in pending.drain(..) {
            let frame_size = (frame.width, frame.height);
            if *size.get_or_insert(frame_size) != frame_size {
                eprintln!(
                    "Warning: frame size changed to {}x{}, the output file mixes sizes.",
                    frame.width, frame.height
                );
                size = Some(frame_size);
            }
            write_i420(&frame, file);
            written += 1;
        }
    };
    for frame in frames {
        if frame.is_idr {
            flush(&mut pending, &mut file);
        }
        pending.push(frame);
    }
    flush(&mut pending, &mut file);
    file.flush().expect("failed to write the output file");

    let (width, height) = size.expect("the stream contained no frames");
    println!("Wrote {written} frames to {output}");
    println!("\nCompare against ffmpeg:");
    println!("  ffmpeg -i {input} -f rawvideo -pix_fmt yuv420p ref.yuv");
    println!(
        "  ffmpeg -f rawvideo -video_size {width}x{height} -pix_fmt yuv420p -i {output} \\\n         -f rawvideo -video_size {width}x{height} -pix_fmt yuv420p -i ref.yuv \\\n         -lavfi psnr -f null -"
    );
}

/// Writes the NV12 frame as planar yuv420p: the luma plane unchanged,
/// the interleaved chroma plane split into its U and V planes.
fn write_i420(frame: &CpuFrame, file: &mut dyn std::io::Write) {
    let luma_size = (frame.width * frame.height) as usize;
    let (luma, chroma) = frame.data.split_at(luma_size);
    file.write_all(luma).expect("write failed");

    let mut plane = Vec::with_capacity(chroma.len() / 2);
    for offset in [0, 1] {
        plane.clear();
        plane.extend(chroma.iter().skip(offset).step_by(2));
        file.write_all(&plane).expect("write failed");
    }
}
