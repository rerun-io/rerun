fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_getting_started")
        .recording_id("run-1")
        .save("run-1.rrd")?;

    for t in 0..10 {
        let t = t as f64;
        rec.set_duration_secs("t", t);
        rec.log("/arm/shoulder", &rerun::Scalars::single((t * 0.5).sin()))?;
        rec.log("/arm/elbow", &rerun::Scalars::single((t * 0.5).cos()))?;
    }

    Ok(())
}
