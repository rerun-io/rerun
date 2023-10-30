for frame in read_sensor_frames():
    rr.set_time_sequence("frame_idx", frame.idx)
    rr.set_time_seconds("sensor_time", frame.timestamp)

    rr.log("sensor/points", rr.Points3D(frame.points))
