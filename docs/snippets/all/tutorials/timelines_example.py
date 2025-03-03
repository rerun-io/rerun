for frame in read_sensor_frames():
    rr.set_index("frame_idx", sequence=frame.idx)
    rr.set_index("sensor_time", datetime=frame.timestamp)

    rr.log("sensor/points", rr.Points3D(frame.points))
