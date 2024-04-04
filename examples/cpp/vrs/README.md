<!--[metadata]
title = "VRS Viewer"
source = "https://github.com/rerun-io/cpp-example-vrs"
tags = ["2D", "3D", "vrs", "viewer", "C++"]
thumbnail = "https://static.rerun.io/vrs-viewer/28da92ebc2f0bccd5cf904314d2f8b0b0c45c879/480w.png"
thumbnail_dimensions = [480, 480]
-->
[//]: # (> VRS is a file format optimized to record & playback streams of sensor data, such as images, audio samples, and any other discrete sensors &#40;IMU, temperature, etc&#41;, stored in per-device streams of time-stamped records.)

[//]: # (You can find the example at <https://github.com/rerun-io/cpp-example-vrs>.)

<picture>
  <img src="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/1024w.png">
</picture>

This is an example that shows how to use Rerun's C++ API to log and view [VRS](https://github.com/facebookresearch/vrs) files.


# Used Rerun Types

[`Arrows3D`](https://www.rerun.io/docs/reference/types/archetypes/arrows3d), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Scalar`](https://www.rerun.io/docs/reference/types/archetypes/scalar), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

# Background
This C++ example demonstrates how to visualize VRS files with Rerun. 
VRS is a file format optimized to record & playback streams of sensor data, such as images, audio samples, and any other discrete sensors (IMU, temperature, etc), stored in per-device streams of time-stamped records. 

# Logging and Visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

## 3D Arrows
```cpp
void IMUPlayer::log_accelerometer(const std::array<float, 3>& accelMSec2) {
        _rec->log(_entity_path + "/accelerometer", rerun::Arrows3D::from_vectors({accelMSec2}));
        // ... existing code for scalars ...
}
```

## Scalars 
```cpp
void IMUPlayer::log_accelerometer(const std::array<float, 3>& accelMSec2) {
        // ... existing code for Arrows3D ...
        _rec->log(_entity_path + "/accelerometer/x", rerun::Scalar(accelMSec2[0]));
        _rec->log(_entity_path + "/accelerometer/y", rerun::Scalar(accelMSec2[1]));
        _rec->log(_entity_path + "/accelerometer/z", rerun::Scalar(accelMSec2[2]));
}
```

```cpp
void IMUPlayer::log_gyroscope(const std::array<float, 3>& gyroRadSec) {
        _rec->log(_entity_path + "/gyroscope/x", rerun::Scalar(gyroRadSec[0]));
        _rec->log(_entity_path + "/gyroscope/y", rerun::Scalar(gyroRadSec[1]));
        _rec->log(_entity_path + "/gyroscope/z", rerun::Scalar(gyroRadSec[2]));
}
```

```cpp
void IMUPlayer::log_magnetometer(const std::array<float, 3>& magTesla) {
        _rec->log(_entity_path + "/magnetometer/x", rerun::Scalar(magTesla[0]));
        _rec->log(_entity_path + "/magnetometer/y", rerun::Scalar(magTesla[1]));
        _rec->log(_entity_path + "/magnetometer/z", rerun::Scalar(magTesla[2]));
}
```

## Images
```cpp
_rec->log(
    _entity_path,
    rerun::Image({
    frame->getHeight(),
    frame->getWidth(),
    frame->getSpec().getChannelCountPerPixel()},
    frame->getBuffer()
    )
);
```

# Run the Code
You can find the build instructions here: [C++ Example: VRS Viewer](https://github.com/rerun-io/cpp-example-vrs)
