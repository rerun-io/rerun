"""Log an encoded depth image stored as a 16-bit PNG."""

from pathlib import Path

import rerun as rr

depth_path = Path(__file__).parent / "encoded_depth.png"

rr.init("rerun_example_encoded_depth_image", spawn=True)

depth_png = depth_path.read_bytes()
depth_format = rr.components.ImageFormat(
    width=64,
    height=48,
    channel_datatype=rr.datatypes.ChannelDatatype.U16,
)

rr.log(
    "depth/encoded",
    rr.EncodedDepthImage(
        blob=depth_png,
        format=depth_format,
        media_type=rr.components.MediaType.PNG,
        meter=0.001,
    ),
)
