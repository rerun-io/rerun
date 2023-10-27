for frame in read_sensor_frames() {
    rec.set_time_sequence("frame_idx", frame.idx);
    rec.set_time_seconds("sensor_time", frame.timestamp);

    rec.log("sensor/points", rerun::Points3D::new(&frame.points))?;
}
