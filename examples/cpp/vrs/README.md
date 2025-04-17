<!--[metadata]
title = "VRS viewer"
source = "https://github.com/rerun-io/cpp-example-vrs"
tags = ["2D", "3D", "VRS", "Viewer", "C++"]
thumbnail = "https://static.rerun.io/vrs/614f0adf0dd31fa01fff0d6eaeae67bbe8ba9af0/480w.png"
thumbnail_dimensions = [480, 482]
-->

<picture>
  <img src="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/cpp-example-vrs/c765460d4448da27bb9ee2a2a15f092f82a402d2/1024w.png">
</picture>

This is an example that shows how to use Rerun's C++ API to log and view [VRS](https://github.com/facebookresearch/vrs) files.


# Used Rerun types

[`Arrows3D`](https://www.rerun.io/docs/reference/types/archetypes/arrows3d), [`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

# Background
This C++ example demonstrates how to visualize VRS files with Rerun.
VRS is a file format optimized to record & playback streams of sensor data, such as images, audio samples, and any other discrete sensors (IMU, temperature, etc), stored in per-device streams of time-stamped records.

# Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

## 3D arrows
```cpp
void IMUPlayer::log_accelerometer(const std::array<float, 3>& accelMSec2) {
    _rec->log(_entity_path + "/accelerometer", rerun::Arrows3D::from_vectors({accelMSec2}));
    // … existing code for scalars …
}
```

## Scalars
```cpp
void IMUPlayer::log_accelerometer(const std::array<float, 3>& accelMSec2) {
    // … existing code for Arrows3D …
    _rec->log(_entity_path + "/accelerometer/x", rerun::Scalars(accelMSec2[0]));
    _rec->log(_entity_path + "/accelerometer/y", rerun::Scalars(accelMSec2[1]));
    _rec->log(_entity_path + "/accelerometer/z", rerun::Scalars(accelMSec2[2]));
}
```

```cpp
void IMUPlayer::log_gyroscope(const std::array<float, 3>& gyroRadSec) {
    _rec->log(_entity_path + "/gyroscope/x", rerun::Scalars(gyroRadSec[0]));
    _rec->log(_entity_path + "/gyroscope/y", rerun::Scalars(gyroRadSec[1]));
    _rec->log(_entity_path + "/gyroscope/z", rerun::Scalars(gyroRadSec[2]));
}
```

```cpp
void IMUPlayer::log_magnetometer(const std::array<float, 3>& magTesla) {
    _rec->log(_entity_path + "/magnetometer/x", rerun::Scalars(magTesla[0]));
    _rec->log(_entity_path + "/magnetometer/y", rerun::Scalars(magTesla[1]));
    _rec->log(_entity_path + "/magnetometer/z", rerun::Scalars(magTesla[2]));
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

## Text document
```cpp
_rec->log_static(_entity_path + "/configuration", rerun::TextDocument(layout_str));
```

# Run the code
You can find the build instructions here: [C++ Example: VRS Viewer](https://github.com/rerun-io/cpp-example-vrs)
