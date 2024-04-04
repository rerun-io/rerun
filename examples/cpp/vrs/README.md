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


[//]: # (# Used Rerun Types)

[//]: # (???)

# Background
This C++ example demonstrates the integration of the Rerun with VRS files. 
VRS is a file format optimized to record & playback streams of sensor data, such as images, audio samples, and any other discrete sensors (IMU, temperature, etc), stored in per-device streams of time-stamped records. 

# Logging and Visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

```cpp
int main(int argc, const char* argv[]) {

    // ... existing code ...

    vrs::RecordFileReader reader;
    if (reader.openFile(vrs_path) == 0) {
        std::vector<std::unique_ptr<vrs::StreamPlayer>> stream_players;
        const std::set<vrs::StreamId>& streamIds = reader.getStreams();
        for (auto id : streamIds) {
            std::cout << id.getName() << " (" << id.getTypeName() << ")" << ": ";
            if (reader.mightContainImages(id)) {
                std::cout << "Handled by FramePlayer" << std::endl;
                stream_players.emplace_back(std::make_unique<rerun_vrs::FramePlayer>(id, rec));
                reader.setStreamPlayer(id, stream_players.back().get());
            } else if (rerun_vrs::might_contain_imu_data(id)) {
                std::cout << "Handled by IMUPlayer" << std::endl;
                stream_players.emplace_back(std::make_unique<rerun_vrs::IMUPlayer>(id, rec));
                reader.setStreamPlayer(id, stream_players.back().get());
            } else {
                std::cout << "No player available. Skipped." << std::endl;
            }
        }
        reader.readAllRecords();
    }
    
    // ... existing code ...
}
```



# Run the Code
You can find the build instructions here: [C++ Example: VRS Viewer](https://github.com/rerun-io/cpp-example-vrs)
