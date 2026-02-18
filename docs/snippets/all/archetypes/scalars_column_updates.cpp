//! Update a scalar over time, in a single operation.
//!
//! This is semantically equivalent to the `scalar_row_updates` example, albeit much faster.

#include <cmath>
#include <numeric>
#include <vector>

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_scalar_column_updates");
    rec.spawn().exit_on_failure();

    // Native scalars & times.
    std::vector<double> scalar_data(64);
    for (size_t i = 0; i < 64; ++i) {
        scalar_data[i] = sin(static_cast<double>(i) / 10.0);
    }
    std::vector<int64_t> times(64);
    std::iota(times.begin(), times.end(), 0);

    // Serialize to columns and send.
    rec.send_columns(
        "scalars",
        rerun::TimeColumn::from_sequence("step", std::move(times)),
        rerun::Scalars(std::move(scalar_data)).columns()
    );
}
