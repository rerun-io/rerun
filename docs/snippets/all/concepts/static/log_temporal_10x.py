rr.set_index("frame", sequence=4)
for _ in range(10):
    rr.log("camera/image", camera.save_current_frame())
