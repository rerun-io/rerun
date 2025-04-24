// Log a video asset using automatically determined frame references.

#include <rerun.hpp>

#include <iostream>

using namespace std::chrono_literals;

int main(int argc, char* argv[]) {
    if (argc < 2) {
        // TODO(#7354): Only mp4 is supported for now.
        std::cerr << "Usage: " << argv[0] << " <path_to_video.[mp4]>" << std::endl;
        return 1;
    }

    const auto path = argv[1];

    const auto rec = rerun::RecordingStream("rerun_example_asset_video_auto_frames");
    rec.spawn().exit_on_failure();

    // Log video asset which is referred to by frame references.
    auto video_asset = rerun::AssetVideo::from_file(path).value_or_throw();
    rec.log_static("video", video_asset);

    // Send automatically determined video frame timestamps.
    std::vector<std::chrono::nanoseconds> frame_timestamps_ns =
        video_asset.read_frame_timestamps_nanos().value_or_throw();
    // Note timeline values don't have to be the same as the video timestamps.
    auto time_column =
        rerun::TimeColumn::from_durations("video_time", rerun::borrow(frame_timestamps_ns));

    std::vector<rerun::components::VideoTimestamp> video_timestamps(frame_timestamps_ns.size());
    for (size_t i = 0; i < frame_timestamps_ns.size(); i++) {
        video_timestamps[i] = rerun::components::VideoTimestamp(frame_timestamps_ns[i]);
    }
    auto video_frame_reference_indicators =
        rerun::ComponentColumn::from_indicators<rerun::VideoFrameReference>(
            static_cast<uint32_t>(video_timestamps.size())
        );

    rec.send_columns(
        "video",
        time_column,
        rerun::VideoFrameReference().with_many_timestamp(rerun::borrow(video_timestamps)).columns()
    );
}
