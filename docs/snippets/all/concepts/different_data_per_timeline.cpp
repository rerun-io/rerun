// Log different data on different timelines.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_different_data_per_timeline");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("blue timeline", 0);
    rec.set_time_duration_secs("red timeline", 0.0);
    rec.log("points", rerun::Points2D({{0.0, 0.0}, {1.0, 1.0}}));

    // Log a red color on one timeline.
    rec.reset_time(); // Clears all set timeline info.
    rec.set_time_duration_secs("red timeline", 1.0);
    rec.log("points", rerun::Points2D::update_fields().with_colors(rerun::Color(0xFF0000FF)));

    // And a blue color on the other.
    rec.reset_time(); // Clears all set timeline info.
    rec.set_time_sequence("blue timeline", 1);
    rec.log("points", rerun::Points2D::update_fields().with_colors(rerun::Color(0x0000FFFF)));

    // TODO(#5521): log VisualBounds2D
}
