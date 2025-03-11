for frame in read_sensor_frames() {
    rec.set_time_sequence("frame_idx", frame.idx);
    rec.set_timestamp_seconds_since_epoch("sensor_time", frame.timestamp);

    rec.log("sensor/points", rerun::Points3D::new(&frame.points))?;
}
