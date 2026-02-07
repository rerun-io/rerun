"""Log a simple MCAP channel definition."""

import rerun as rr

rr.init("rerun_example_mcap_channel", spawn=True)

rr.log(
    "mcap/channels/camera",
    rr.McapChannel(
        id=1,
        topic="/camera/image",
        message_encoding="cdr",
        metadata={"frame_id": "camera_link", "encoding": "bgr8"},
    ),
)
