// Log a video asset using automatically determined frame references.
// TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

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

    const auto rec = rerun::RecordingStream("rerun_example_asset_video_manual_frames");
    rec.spawn().exit_on_failure();

    // Log video asset which is referred to by frame references.
    // Make sure it's available on the timeline used for the frame references.
    rec.set_time_seconds("video_time", 0.0);
    auto video_asset = rerun::AssetVideo::from_file(path).value_or_throw();
    rec.log("video", video_asset);

    // Send frame references for every 0.1 seconds over a total of 10 seconds.
    // Naturally, this will result in a choppy playback and only makes sense if the video is 10 seconds or longer.
    // TODO(#7368): Point to example using `send_video_frames`.
    //
    // Use `send_columns` to send all frame references in a single call.
    std::vector<std::chrono::nanoseconds> times =
        video_asset.read_frame_timestamps_ns().value_or_throw();
    std::vector<rerun::components::VideoTimestamp> video_timestamps(times.size());
    for (size_t i = 0; i < times.size(); i++) {
        video_timestamps[i] = rerun::components::VideoTimestamp(times[i]);
    }
    auto video_frame_reference_indicators =
        rerun::ComponentColumn::from_indicators<rerun::VideoFrameReference>(
            static_cast<uint32_t>(times.size())
        );
    rec.send_columns(
        "video",
        rerun::TimeColumn::from_times("video_time", rerun::borrow(times)),
        {
            video_frame_reference_indicators.value_or_throw(),
            rerun::ComponentColumn::from_loggable(rerun::borrow(video_timestamps)).value_or_throw(),
        }
    );
}
