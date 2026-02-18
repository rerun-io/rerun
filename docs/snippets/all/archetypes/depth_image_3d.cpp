// Create and log a depth image and pinhole camera.

#include <rerun.hpp>

#include <algorithm> // fill_n
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_depth_image_3d");
    rec.spawn().exit_on_failure();

    // Create a synthetic depth image.
    const int HEIGHT = 200;
    const int WIDTH = 300;
    std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
    for (auto y = 50; y < 150; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 50, 100, static_cast<uint16_t>(20000));
    }
    for (auto y = 130; y < 180; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 100, 180, static_cast<uint16_t>(45000));
    }

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    rec.log(
        "world/camera",
        rerun::Pinhole::from_focal_length_and_resolution(
            200.0f,
            {static_cast<float>(WIDTH), static_cast<float>(HEIGHT)}
        )
    );

    rec.log(
        "world/camera/depth",
        rerun::DepthImage(data.data(), {WIDTH, HEIGHT})
            .with_meter(10000.0)
            .with_colormap(rerun::components::Colormap::Viridis)
    );
}
