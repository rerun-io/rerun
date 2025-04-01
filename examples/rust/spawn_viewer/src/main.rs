//! Spawn a new Rerun Viewer process ready to listen for gRPC connections.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rerun::spawn(&rerun::SpawnOptions::default())?;
    Ok(())
}
