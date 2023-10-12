#include <optional>
#include <rerun/archetypes/tensor.hpp>
#include <rerun/datatypes/tensor_data.hpp>
#include <rerun/recording_stream.hpp>

int main(int argc, char** argv) {
    auto rec = rerun::RecordingStream("rerun_example_roundtrip_tensor");
    rec.save(argv[1]).throw_on_failure();

    std::vector<rerun::datatypes::TensorDimension> dimensions{
        rerun::datatypes::TensorDimension{3, std::nullopt},
        rerun::datatypes::TensorDimension{4, std::nullopt},
        rerun::datatypes::TensorDimension{5, std::nullopt},
        rerun::datatypes::TensorDimension{6, std::nullopt}
    };

    std::vector<int32_t> data;
    for (auto i = 0; i < 360; ++i) {
        data.push_back(i);
    }

    // TODO(jleibs) Tensor data can't actually be logged yet because C++ Unions
    // don't supported nested list-types.
    rec.log(
        "tensor",
        rerun::archetypes::Tensor(
            rerun::datatypes::TensorData{dimensions, rerun::datatypes::TensorBuffer::i32(data)}
        )
    );
}
