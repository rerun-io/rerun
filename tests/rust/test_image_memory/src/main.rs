//! Logs a bunch of big images to test Rerun memory usage.

use mimalloc::MiMalloc;

use re_memory::AccountingAllocator;
use rerun::external::{image, re_memory, re_viewer};

#[global_allocator]
static GLOBAL: AccountingAllocator<MiMalloc> = AccountingAllocator::new(MiMalloc);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    re_memory::accounting_allocator::turn_on_tracking_if_env_var(
        re_viewer::env_vars::RERUN_TRACK_ALLOCATIONS,
    );

    let recording_info = rerun::new_recording_info("test_image_memory_rs");
    rerun::native_viewer::spawn(recording_info, |session| {
        log_images(&session).unwrap();
    })?;
    Ok(())
}

fn log_images(session: &rerun::Session) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = (2048, 1024);
    let n = 100;

    let image = image::RgbaImage::from_fn(w, h, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgba([0, 0, 0, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    });
    let tensor = rerun::components::Tensor::from_image(image)?;

    for _ in 0..n {
        rerun::MsgSender::new("image")
            .with_component(&[tensor.clone()])?
            .send(session)?;
    }

    eprintln!(
        "Logged {n} {w}x{h} RGBA images = {}",
        re_format::format_bytes((n * w * h * 4) as _)
    );

    // Give viewer time to load it:
    std::thread::sleep(std::time::Duration::from_secs(2));

    if let Some(allocs) = re_memory::accounting_allocator::global_allocs() {
        eprintln!("{} RAM used", re_format::format_bytes(allocs.size as _));
    }

    Ok(())
}
