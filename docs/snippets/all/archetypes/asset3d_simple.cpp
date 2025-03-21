// Log a simple 3D asset.

#include <rerun.hpp>

#include <iostream>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <path_to_asset.[gltf|glb|obj|stl]>" << std::endl;
        return 1;
    }

    const auto path = argv[1];

    const auto rec = rerun::RecordingStream("rerun_example_asset3d");
    rec.spawn().exit_on_failure();

    rec.log_static("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis
    rec.log("world/asset", rerun::Asset3D::from_file_path(path).value_or_throw());
}
