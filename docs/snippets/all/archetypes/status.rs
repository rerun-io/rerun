//! Log a `Status`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_status").spawn()?;

    rec.set_time_sequence("step", 0);
    rec.log("door", &rerun::Status::new().with_status("open"))?;

    rec.set_time_sequence("step", 1);
    rec.log("door", &rerun::Status::new().with_status("closed"))?;

    rec.set_time_sequence("step", 2);
    rec.log("door", &rerun::Status::new().with_status("open"))?;

    Ok(())
}
