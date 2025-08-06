// Log a video asset using manually created frame references.

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
    rec.log_static("video_asset", rerun::AssetVideo::from_file(path).value_or_throw());

    // Create two entities, showing the same video frozen at different times.
    rec.log("frame_1s", rerun::VideoFrameReference(1.0s).with_video_reference("video_asset"));
    rec.log("frame_2s", rerun::VideoFrameReference(2.0s).with_video_reference("video_asset"));

    // TODO(#5520): log blueprint once supported
}
