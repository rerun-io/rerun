import depthai_viewer as viewer
import cv2
import depthai as dai
import queue

viewer.init("Depthai Viewer")
viewer.connect()

# Create pipeline
pipeline = dai.Pipeline()
color = pipeline.createColorCamera()
color.setResolution(dai.ColorCameraProperties.SensorResolution.THE_1080_P)
color.setFps(60)

# Create output
xout = pipeline.createXLinkOut()
xout.setStreamName("color")
color.video.link(xout.input)

q = queue.Queue(maxsize=4)


def color_frame_callback(frame: dai.ImgFrame):
    global q
    q.put(frame.getCvFrame())


import time

# Connect to device and start pipeline
with dai.Device(pipeline) as device:
    q = device.getOutputQueue("color", maxSize=4, blocking=False)  # .addCallback(color_frame_callback)
    t = time.time_ns()
    fps = 0
    display_fps = 0
    while True:
        # in_color = q.get()
        # cv2.imshow("color", in_color)
        in_color = q.get()
        fps += 1
        if time.time_ns() - t > 1e9:
            display_fps = fps
            fps = 0
            print("fps: ", display_fps)
            t = time.time_ns()
        frame = in_color.getCvFrame()
        # cv2.putText(frame, f"fps: {display_fps:.2f}", (10, 30), cv2.FONT_HERSHEY_SIMPLEX, 1, (255, 0, 0), 2)
        # cv2.imshow("color", frame)
        viewer.log_image("color/camera/rgb/Color camera", cv2.cvtColor(frame, cv2.COLOR_BGR2RGB))
        cv2.waitKey(1)
