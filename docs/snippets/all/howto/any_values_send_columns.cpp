// Use `send_column` to send entire columns of custom data to Rerun.

#include <rerun.hpp>

#include <arrow/array/builder_primitive.h>
#include <cmath>
#include <cstdio>
#include <numeric>

arrow::Status run_main() {
    const auto rec = rerun::RecordingStream("rerun_example_any_values_send_columns");
    rec.spawn().exit_on_failure();

    constexpr int64_t STEPS = 64;

    std::vector<int64_t> times(STEPS);
    std::iota(times.begin(), times.end(), 0);

    std::shared_ptr<arrow::Array> arrow_array;

    arrow::DoubleBuilder sin_builder;
    for (int64_t i = 0; i < STEPS; i++) {
        ARROW_RETURN_NOT_OK(sin_builder.Append(sin(static_cast<double>(i) / 10.0)));
    }
    ARROW_RETURN_NOT_OK(sin_builder.Finish(&arrow_array));
    auto sin =
        rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "sin").value_or_throw();

    arrow::DoubleBuilder cos_builder;
    for (int64_t i = 0; i < STEPS; i++) {
        ARROW_RETURN_NOT_OK(cos_builder.Append(cos(static_cast<double>(i) / 10.0)));
    }
    ARROW_RETURN_NOT_OK(cos_builder.Finish(&arrow_array));
    auto cos =
        rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "cos").value_or_throw();

    rec.send_columns(
        "/",
        rerun::TimeColumn::from_sequence_points("step", std::move(times)),
        sin.partitioned().value_or_throw(),
        cos.partitioned().value_or_throw()
    );

    return arrow::Status::OK();
}

int main() {
    arrow::Status status = run_main();
    if (!status.ok()) {
        printf("%s\n", status.ToString().c_str());
        return 1;
    }
    return 0;
}
