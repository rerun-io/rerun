// Set different types of indices.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_different_indices");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("frame_nr", 42);
    rec.set_time_duration_secs("elapsed", 12.0);
    rec.set_time_timestamp_secs_since_epoch("time", 1'741'017'564);
    rec.set_time_timestamp_nanos_since_epoch("precise_time", 1'741'017'564'987'654'000);

    // All following logged data will be timestamped with the above times:
    rec.log("points", rerun::Points2D({{0.0, 0.0}, {1.0, 1.0}}));
}
