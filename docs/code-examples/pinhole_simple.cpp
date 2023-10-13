// Log a pinhole and a random image.

#include <rerun.hpp>

#include <algorithm>
#include <cstdlib>
#include <ctime>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_strip3d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("world/image", rerun::Pinhole::focal_length_and_resolution({3.0f, 3.0f}, {3.0f, 3.0f}));

    // TODO(andreas): Improve ergonomics.
    rerun::datatypes::TensorData tensor;
    rerun::datatypes::TensorDimension dim3;
    dim3.size = 3;
    tensor.shape = {dim3, dim3, dim3};
    std::srand(static_cast<uint32_t>(std::time(nullptr)));
    std::vector<uint8_t> random_data(3 * 3 * 3);
    std::generate(random_data.begin(), random_data.end(), std::rand);
    tensor.buffer = rerun::datatypes::TensorBuffer::u8(random_data);

    rec.log("world/image", rerun::Image(tensor));
}
