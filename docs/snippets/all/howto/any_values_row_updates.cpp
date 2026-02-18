//! Update custom user-defined values over time.
//!
//! See also the `any_values_column_updates` example, which achieves the same thing in a single operation.

#include <rerun.hpp>

#include <arrow/array/builder_primitive.h>
#include <cmath>
#include <cstdio>

arrow::Status run_main() {
    const auto rec = rerun::RecordingStream("rerun_example_any_values_row_updates");
    rec.spawn().exit_on_failure();

    for (int64_t i = 0; i < 64; i++) {
        rec.set_time_sequence("step", i);

        std::shared_ptr<arrow::Array> arrow_array;

        arrow::DoubleBuilder sin_builder;
        ARROW_RETURN_NOT_OK(sin_builder.Append(sin(static_cast<double>(i) / 10.0)));
        ARROW_RETURN_NOT_OK(sin_builder.Finish(&arrow_array));
        auto sin =
            rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "sin").value_or_throw();

        arrow::DoubleBuilder cos_builder;
        ARROW_RETURN_NOT_OK(cos_builder.Append(cos(static_cast<double>(i) / 10.0)));
        ARROW_RETURN_NOT_OK(cos_builder.Finish(&arrow_array));
        auto cos =
            rerun::ComponentBatch::from_arrow_array(std::move(arrow_array), "cos").value_or_throw();

        rec.log("/", sin, cos);
    }

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
