// Log a simple 3D asset.

#include <rerun.hpp>

#include <filesystem>
#include <iostream>
#include <string>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <path_to_asset.[gltf|glb|obj|stl]>" << std::endl;
        return 1;
    }

    const auto path = argv[1];

    const auto rec = rerun::RecordingStream("rerun_example_asset_video");
    rec.spawn().exit_on_failure();

    rec.log("world/video", rerun::AssetVideo::from_file(path).value_or_throw());
}
