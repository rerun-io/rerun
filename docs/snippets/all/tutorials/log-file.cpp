const auto rec = rerun::RecordingStream("rerun_example_log_file");
rec.spawn().exit_on_failure();

rec.log_file_from_path(argv[1]);
