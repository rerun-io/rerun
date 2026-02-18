// Use `send_column` to send an entire column of custom data to Rerun.

#include <rerun.hpp>

#include <arrow/array/builder_primitive.h>
#include <cmath>
#include <cstdio>
#include <numeric>

arrow::Status run_main() {
    const auto rec = rerun::RecordingStream("rerun_example_any_batch_value_column_updates");
    rec.spawn().exit_on_failure();

    constexpr int64_t STEPS = 64;

    std::vector<int64_t> times(STEPS);
    std::iota(times.begin(), times.end(), 0);

    std::shared_ptr<arrow::Array> arrow_array;

    arrow::DoubleBuilder one_per_timestamp_builder;
    for (int64_t i = 0; i < STEPS; i++) {
        ARROW_RETURN_NOT_OK(one_per_timestamp_builder.Append(sin(static_cast<double>(i) / 10.0)));
    }
    ARROW_RETURN_NOT_OK(one_per_timestamp_builder.Finish(&arrow_array));
    auto one_per_timestamp =
        rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "custom_component_single")
            .value_or_throw();

    arrow::DoubleBuilder ten_per_timestamp_builder;
    for (int64_t i = 0; i < STEPS * 10; i++) {
        ARROW_RETURN_NOT_OK(ten_per_timestamp_builder.Append(cos(static_cast<double>(i) / 100.0)));
    }
    ARROW_RETURN_NOT_OK(ten_per_timestamp_builder.Finish(&arrow_array));
    auto ten_per_timestamp =
        rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "custom_component_multi")
            .value_or_throw();

    rec.send_columns(
        "/",
        rerun::TimeColumn::from_sequence("step", std::move(times)),
        one_per_timestamp.partitioned().value_or_throw(),
        ten_per_timestamp.partitioned(std::vector<uint32_t>(STEPS, 10)).value_or_throw()
    );

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
