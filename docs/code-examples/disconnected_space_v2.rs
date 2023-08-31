//! Disconnect two spaces.

use rerun::{
    archetypes::{DisconnectedSpace, Points3D},
    MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        RecordingStreamBuilder::new("rerun_example_disconnected_space").memory()?;

    // These two points can be projected into the same space..
    MsgSender::from_archetype("world/room1/point", &Points3D::new([(0.0, 0.0, 0.0)]))?
        .send(&rec)?;
    MsgSender::from_archetype("world/room2/point", &Points3D::new([(1.0, 1.0, 1.0)]))?
        .send(&rec)?;

    // ..but this one lives in a completely separate space!
    MsgSender::from_archetype("world/wormhole", &DisconnectedSpace::new(true))?.send(&rec)?;
    MsgSender::from_archetype("world/wormhole/point", &Points3D::new([(2.0, 2.0, 2.0)]))?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;

    Ok(())
}
