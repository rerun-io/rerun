for frame in read_sensor_frames() {
    rec.set_index("frame_idx", sequence=frame.idx);
    rec.set_time_seconds("sensor_time", frame.timestamp);

    rec.log("sensor/points", rerun::Points3D::new(&frame.points))?;
}
