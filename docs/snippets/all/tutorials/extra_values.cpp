// Log extra values with a `Points2D`.

#include <arrow/array/builder_primitive.h>
#include <cstdio>
#include <rerun.hpp>

arrow::Status run_main() {
    const auto rec = rerun::RecordingStream("rerun_example_extra_values");
    rec.spawn().exit_on_failure();

    auto points = rerun::Points2D({{-1.0f, -1.0f}, {-1.0f, 1.0f}, {1.0f, -1.0f}, {1.0f, 1.0f}});

    std::shared_ptr<arrow::Array> arrow_array;
    arrow::DoubleBuilder confidences_builder;
    ARROW_RETURN_NOT_OK(confidences_builder.AppendValues({0.3, 0.4, 0.5, 0.6}));
    ARROW_RETURN_NOT_OK(confidences_builder.Finish(&arrow_array));
    auto confidences =
        rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "confidence");

    rec.log("extra_values", points, confidences);

    return arrow::Status::OK();
}

int main(int argc, char* argv[]) {
    arrow::Status status = run_main();
    if (!status.ok()) {
        printf("%s\n", status.ToString().c_str());
        return 1;
    }
    return 0;
}
