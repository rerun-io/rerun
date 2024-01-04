// File used for snippets that are embedded in the documentation.
// Compiled as part of the tests to make sure everything keeps working!

#include <rerun.hpp>
#include <vector>

static std::vector<rerun::Position3D> create_positions() {
    return {};
}

static std::vector<rerun::Color> create_colors() {
    return {};
}

static std::vector<uint8_t> create_image() {
    return {};
}

[[maybe_unused]] static void log() {
    /// [Logging]
    // Create a recording stream.
    rerun::RecordingStream rec("rerun_example_app");
    // Spawn the viewer and connect to it.
    rec.spawn().exit_on_failure();

    std::vector<rerun::Position3D> points = create_positions();
    std::vector<rerun::Color> colors = create_colors();
    std::vector<uint8_t> image_data = create_image();

    // Log a batch of points.
    rec.log("path/to/points", rerun::Points3D(points).with_colors(colors));

    // Log an image.
    rec.log("path/to/image", rerun::Image({786, 1024, 3}, image_data));
    /// [Logging]
}

[[maybe_unused]] static void streaming() {
    /// [Streaming]
    rerun::RecordingStream rec("rerun_example_app");
    rec.save("example.rrd").exit_on_failure();
    /// [Streaming]
}

[[maybe_unused]] static void connecting() {
    /// [Connecting]
    rerun::RecordingStream rec("rerun_example_app");
    auto result = rec.connect(); // Connect to local host with default port.
    if (result.is_err()) {
        // Handle error.
    }
    /// [Connecting]
}

[[maybe_unused]] static void buffering() {
    std::vector<rerun::Position3D> points = create_positions();
    std::vector<rerun::Color> colors = create_colors();

    /// [Buffering]
    rerun::RecordingStream rec("rerun_example_app");

    // Log data to the internal buffer.
    rec.log("path/to/points", rerun::Points3D(points).with_colors(colors));

    // Spawn & connect later.
    auto result = rec.spawn();
    if (result.is_err()) {
        // Handle error.
    }
    /// [Buffering]
}
