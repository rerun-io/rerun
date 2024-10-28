for _ in range(10):
    rr.log("camera/image", static=True, camera.save_current_frame())
