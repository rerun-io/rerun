#include <algorithm>
#include <optional>
#include <rerun/archetypes/tensor.hpp>
#include <rerun/datatypes/tensor_data.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    auto rec = rerun::RecordingStream("rerun_example_roundtrip_tensor");
    rec.save(argv[1]).throw_on_failure();

    std::vector<rerun::datatypes::TensorDimension> dimensions{{3, 4, 5, 6}};

    std::vector<int32_t> data(360);
    std::generate(data.begin(), data.end(), [n = 0]() mutable { return n++; });

    rec.log(
        "tensor",
        rerun::archetypes::Tensor(
            rerun::datatypes::TensorData{dimensions, rerun::datatypes::TensorBuffer::i32(data)}
        )
    );
}
