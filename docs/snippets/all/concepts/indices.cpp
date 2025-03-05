// Set different types of indices.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_different_indices");
    rec.spawn().exit_on_failure();

    rec.set_index_sequence("frame_nr", 42);
    rec.set_time_seconds("elapsed", 12.0);
    rec.set_time_seconds("time", 1'741'017'564);                   // seconds since unix epoch
    rec.set_time_nanos("precise_time", 1'741'017'564'987'654'321); // Nanos since unix epoch

    // All following logged data will be timestamped with the above times:
    rec.log("points", rerun::Points2D({{0.0, 0.0}, {1.0, 1.0}}));
}
