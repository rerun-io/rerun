#include <rerun/blueprint/archetypes/visible_time_ranges.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_visible_time_ranges");
    rec.save(argv[1]).exit_on_failure();

    rerun::datatypes::VisibleTimeRange range0;
    range0.timeline = "timeline0";
    range0.range.start = rerun::datatypes::TimeRangeBoundary::infinite();
    range0.range.end = rerun::datatypes::TimeRangeBoundary::cursor_relative(-10);

    rerun::datatypes::VisibleTimeRange range1;
    range1.timeline = "timeline1";
    range1.range.start = rerun::datatypes::TimeRangeBoundary::cursor_relative(20);
    range1.range.end = rerun::datatypes::TimeRangeBoundary::infinite();

    rerun::datatypes::VisibleTimeRange range2;
    range2.timeline = "timeline2";
    range2.range.start = rerun::datatypes::TimeRangeBoundary::absolute(20);
    range2.range.end = rerun::datatypes::TimeRangeBoundary::absolute(40);

    rec.log(
        "visible_time_ranges",
        rerun::blueprint::archetypes::VisibleTimeRanges({range0, range1, range2})
    );
}
