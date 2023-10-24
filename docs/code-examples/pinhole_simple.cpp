// Log a pinhole and a random image.

#include <rerun.hpp>

#include <algorithm>
#include <cstdlib>
#include <ctime>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_strip3d");
    rec.connect().throw_on_failure();

    rec.log("world/image", rerun::Pinhole::focal_length_and_resolution(3.0f, {3.0f, 3.0f}));

    std::srand(static_cast<uint32_t>(std::time(nullptr)));
    std::vector<uint8_t> random_data(3 * 3 * 3);
    std::generate(random_data.begin(), random_data.end(), std::rand);

    const auto tensor = rerun::datatypes::TensorData({3, 3, 3}, random_data);
    rec.log("world/image", rerun::Image(tensor));
}
