// Log an audio file at t=0 on the `time` timeline.

#include <rerun.hpp>

#include <iostream>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0]
                  << " <path_to_audio.[aac|flac|m4a|mp3|ogg|wav]>" << std::endl;
        return 1;
    }

    const auto path = argv[1];

    const auto rec = rerun::RecordingStream("rerun_example_asset_audio");
    rec.spawn().exit_on_failure();

    rec.set_time_duration_secs("time", 0.0);
    rec.log("audio", rerun::AssetAudio::from_file_path(path).value_or_throw());
}
