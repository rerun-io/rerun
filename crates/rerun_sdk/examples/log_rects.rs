use arrow2::array::Array;
use arrow2_convert::{serialize::TryIntoArrow, ArrowField};
use clap::Parser;

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

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// Connect to an external viewer
    #[clap(long)]
    connect: bool,

    /// External Address
    #[clap(long)]
    addr: Option<String>,
}

fn main() {
    // Make sure rerun logging goes to stdout
    tracing_subscriber::fmt::init();

    let mut sdk = rerun::Sdk::global();

    // Arg-parsing boiler-plate
    let args = Args::parse();

    // Connect if requested
    if args.connect {
        let addr = if let Some(addr) = &args.addr {
            addr.parse()
        } else {
            Ok(re_sdk_comms::default_server_addr())
        };

        match addr {
            Ok(addr) => {
                sdk.connect(addr);
            }
            Err(err) => {
                panic!("Bad address: {:?}. {:?}", args.addr, err);
            }
        }
    }

    // Capture the log_time and object_path
    let time_point = rerun::log_time();
    let obj_path = ObjPath::from("world/points");

    // Build up some rect data into an arrow array
    let rects = build_some_rects(5);
    let colors = build_some_colors(5);
    let array = rerun::components_as_struct_array(&[("rect", rects), ("color_rgba", colors)]);

    // Create and send the message to the sdk
    let msg = rerun::build_arrow_log_msg(&obj_path, &array, &time_point).unwrap();
    sdk.send(msg);

    // If not connected, show the GUI inline
    if args.connect {
        sdk.flush();
    } else {
        let log_messages = sdk.drain_log_messages_buffer();
        sdk.show(log_messages);
    }
}
