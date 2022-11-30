use arrow2::array::Array;
use arrow2_convert::{serialize::TryIntoArrow, ArrowField};
use re_log_types::ObjPath;

use rerun_sdk as rerun;

// Setup the rerun allocator
use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

// TODO(jleibs):
// Move these to definition in `re_arrow_store` after https://github.com/rerun-io/rerun/pull/415 lands
////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, PartialEq, ArrowField)]
pub struct Rect2D {
    /// Rect X-coordinate
    pub x: f32,
    /// Rect Y-coordinate
    pub y: f32,
    /// Box Width
    pub w: f32,
    /// Box Height
    pub h: f32,
}

#[derive(Debug, PartialEq, ArrowField)]
pub struct Point2D {
    x: f32,
    y: f32,
}

#[derive(Debug, PartialEq, ArrowField)]
pub struct Point3D {
    x: f32,
    y: f32,
    z: f32,
}

type ColorRGBA = u32;
////////////////////////////////////////////////////////////////////////////////

/// Create `len` dummy rectangles
fn build_some_rects(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| Rect2D {
            x: i as f32,
            y: i as f32,
            w: (i / 2) as f32,
            h: (i / 2) as f32,
        })
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Create `len` dummy colors
fn build_some_colors(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| i as ColorRGBA)
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

fn main() {
    tracing_subscriber::fmt::init(); // log to stdout
    re_log::info!("Log points example!");

    let mut sdk = rerun::Sdk::global();

    // Timestamp
    let time_point = rerun::log_time();

    // Object-path
    let obj_path = ObjPath::from("world/points");

    // Build the StructArray of components

    let rects = build_some_rects(5);
    let colors = build_some_colors(5);

    let array = rerun::components_as_struct_array(&[("rect", rects), ("color_rgba", colors)]);

    let msg = rerun::build_arrow_log_msg(&obj_path, &array, &time_point)
        .ok()
        .unwrap();

    sdk.send(msg);

    let log_messages = sdk.drain_log_messages_buffer();
    sdk.show(log_messages);
}
