for (auto frame : read_sensor_frames()) {
    rec.set_index_sequence("frame_idx", frame.idx);
    rec.set_time("sensor_time", frame.timestamp);

    rec.log("sensor/points", rerun::Points3D(&frame.points));
}
