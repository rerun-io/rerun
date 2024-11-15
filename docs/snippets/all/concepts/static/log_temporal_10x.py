rr.set_time_sequence("frame", 4)
for _ in range(10):
    rr.log("camera/image", camera.save_current_frame())
