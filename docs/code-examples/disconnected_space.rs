//! Disconnect two spaces.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_disconnected_space").spawn()?;

    // These two points can be projected into the same space..
    rec.log(
        "world/room1/point",
        &rerun::Points3D::new([(0.0, 0.0, 0.0)]),
    )?;
    rec.log(
        "world/room2/point",
        &rerun::Points3D::new([(1.0, 1.0, 1.0)]),
    )?;

    // ..but this one lives in a completely separate space!
    rec.log("world/wormhole", &rerun::DisconnectedSpace::new(true))?;
    rec.log(
        "world/wormhole/point",
        &rerun::Points3D::new([(2.0, 2.0, 2.0)]),
    )?;

    Ok(())
}
