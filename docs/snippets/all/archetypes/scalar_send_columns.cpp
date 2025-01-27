// Use the `send_columns` API to send scalars over time in a single call.

#include <cmath>
#include <numeric>
#include <vector>

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_scalar_send_columns");
    rec.spawn().exit_on_failure();

    // Native scalars & times.
    std::vector<double> scalar_data(64);
    for (size_t i = 0; i < 64; ++i) {
        scalar_data[i] = sin(static_cast<double>(i) / 10.0);
    }
    std::vector<int64_t> times(64);
    std::iota(times.begin(), times.end(), 0);

    // Serialize to columns and send.
    rec.send_columns2(
        "scalars",
        rerun::TimeColumn::from_sequence_points("step", std::move(times)),
        rerun::Scalar().with_many_scalar(std::move(scalar_data)).columns()
    );
}
