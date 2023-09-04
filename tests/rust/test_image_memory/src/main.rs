//! Logs a bunch of big images to test Rerun memory usage.

use mimalloc::MiMalloc;

use re_memory::AccountingAllocator;
use rerun::{
    archetypes::Image,
    datatypes::TensorData,
    external::{image, re_memory, re_viewer},
};

#[global_allocator]
static GLOBAL: AccountingAllocator<MiMalloc> = AccountingAllocator::new(MiMalloc);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    re_memory::accounting_allocator::turn_on_tracking_if_env_var(
        re_viewer::env_vars::RERUN_TRACK_ALLOCATIONS,
    );

    let store_info = rerun::new_store_info("test_image_memory_rs");
    rerun::native_viewer::spawn(store_info, Default::default(), |rec| {
        log_images(&rec).unwrap();
    })?;
    Ok(())
}

fn log_images(rec: &rerun::RecordingStream) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = (2048, 1024);
    let n = 100;

    let image = image::RgbaImage::from_fn(w, h, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgba([0, 0, 0, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    });

    for _ in 0..n {
        rec.log("image", &Image::new(TensorData::from_image(image.clone())?))?;
    }

    rec.flush_blocking();

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
