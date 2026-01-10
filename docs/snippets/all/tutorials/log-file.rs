    let args = std::env::args().collect::<Vec<_>>();

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_log_file").spawn()?;

    rec.log_file_from_path(&args[1], None /* prefix */, None /* transform_frame_prefix */, true /* static */)?;
