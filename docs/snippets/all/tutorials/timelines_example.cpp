for (auto frame : read_sensor_frames()) {
    rec.set_index("frame_idx", sequence=frame.idx);
    rec.set_time("sensor_time", frame.timestamp);

    rec.log("sensor/points", rerun::Points3D(&frame.points));
}
