// Set different types of indices.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_different_indices");
    rec.spawn().exit_on_failure();

    rec.set_index_sequence("frame_nr", 42);
    rec.set_index_duration_secs("elapsed", 12.0);
    rec.set_index_timestamp_seconds_since_epoch("time", 1'741'017'564);
    rec.set_index_timestamp_nanos_since_epoch("precise_time", 1'741'017'564'987'654'321);

    // All following logged data will be timestamped with the above times:
    rec.log("points", rerun::Points2D({{0.0, 0.0}, {1.0, 1.0}}));
}
