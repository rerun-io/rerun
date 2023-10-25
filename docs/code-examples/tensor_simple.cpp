// Create and log a tensor.

#include <rerun.hpp>

#include <random>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_tensor_simple");
    rec.connect().throw_on_failure();

    std::default_random_engine gen;
    // On MSVC uint8_t distributions are not supported.
    std::uniform_int_distribution<int> dist(0, 255);

    std::vector<uint8_t> data(8 * 6 * 3 * 5);
    std::generate(data.begin(), data.end(), [&] { return static_cast<uint8_t>(dist(gen)); });

    rec.log(
        "tensor",
        rerun::Tensor({8, 6, 3, 5}, data).with_dim_names({"batch", "channel", "height", "width"})
    );
}
