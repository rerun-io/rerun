//! Disconnect two spaces.

use rerun::{DisconnectedSpace, Points3D, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_disconnected_space").memory()?;

    // These two points can be projected into the same space..
    rec.log("world/room1/point", &Points3D::new([(0.0, 0.0, 0.0)]))?;
    rec.log("world/room2/point", &Points3D::new([(1.0, 1.0, 1.0)]))?;

    // ..but this one lives in a completely separate space!
    rec.log("world/wormhole", &DisconnectedSpace::new(true))?;
    rec.log("world/wormhole/point", &Points3D::new([(2.0, 2.0, 2.0)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
