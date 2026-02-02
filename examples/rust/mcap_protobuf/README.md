<!--[metadata]
title = "Convert and send custom protobuf messages from an MCAP file to Rerun"
channel = "main"
include_in_manifest = true
-->

Demonstrates how to read, convert and send custom Protobuf messages from [MCAP](https://mcap.dev/) to Rerun.
This is achieved by using the Lenses API, which allows transform custom data columns to Rerun components.

> ⚠️ The Lenses API is in active development and thus unstable.

In this particular example, we load and transform Foxglove messages from an MCAP file and log them to Rerun.
Some message types that are supported in this demo:

* [`foxglove.CameraCalibration`](https://docs.foxglove.dev/docs/sdk/schemas/camera-calibration)
* [`foxglove.CompressedImage`](https://docs.foxglove.dev/docs/sdk/schemas/compressed-image)
* [`foxglove.CompressedVideo`](https://docs.foxglove.dev/docs/sdk/schemas/compressed-video)
* [`foxglove.FrameTransforms`](https://docs.foxglove.dev/docs/sdk/schemas/frame-transforms)
* [`foxglove.PoseInFrame`](https://docs.foxglove.dev/docs/sdk/schemas/pose-in-frame)
* [`foxglove.PosesInFrame`](https://docs.foxglove.dev/docs/sdk/schemas/poses-in-frame)

You can run the example with:

```bash
cargo run -p mcap_protobuf /path/to/some.mcap
```

In case you don't have a suitable MCAP file, you can also find instructions for generating a dummy dataset [here](https://foxglove.dev/blog/using-the-foxglove-sdk-to-generate-mcap).
