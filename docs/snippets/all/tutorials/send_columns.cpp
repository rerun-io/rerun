//! Very minimal test of using the send columns APIs.

#include <cmath>
#include <numeric>
#include <vector>

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_send_columns");
    rec.spawn().exit_on_failure();

    // Native time / scalars
    std::vector<int64_t> timeline_values(64);
    std::iota(timeline_values.begin(), timeline_values.end(), 0);
    std::vector<double> scalar_data(64);
    std::transform(
        timeline_values.begin(),
        timeline_values.end(),
        scalar_data.begin(),
        [](int64_t time) { return sin(static_cast<double>(time) / 10.0); }
    );

    // Convert to rerun time / scalars
    auto time_column = rerun::TimeColumn::from_sequence_points("step", std::move(timeline_values));
    auto scalar_data_collection =
        rerun::Collection<rerun::components::Scalar>(std::move(scalar_data));
    auto batch =
        rerun::PartitionedComponentBatch::from_loggable(scalar_data_collection).value_or_throw();

    rec.send_columns("scalars", time_column, batch);
}
