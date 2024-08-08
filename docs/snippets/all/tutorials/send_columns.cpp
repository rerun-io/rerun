// Use the `send_columns` API to send scalars over time in a single call.

#include <cmath>
#include <numeric>
#include <vector>

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_send_columns");
    rec.spawn().exit_on_failure();

    // Native scalars & times.
    std::vector<double> scalar_data(64);
    for (size_t i = 0; i < 64; ++i) {
        scalar_data[i] = sin(static_cast<double>(i) / 10.0);
    }
    std::vector<int64_t> times(64);
    std::iota(times.begin(), times.end(), 0);

    // Convert to rerun time / scalars
    auto time_column = rerun::TimeColumn::from_sequence_points("step", std::move(times));
    auto scalar_data_collection =
        rerun::Collection<rerun::components::Scalar>(std::move(scalar_data));

    rec.send_columns("scalars", time_column, scalar_data_collection);
}
