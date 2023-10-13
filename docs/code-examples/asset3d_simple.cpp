// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

int main(int argc, char* argv[]) {
    std::vector<std::string> args(argv, argv + argc);

    if (args.size() < 2) {
        std::cerr << "Usage: " << args[0] << " <path_to_asset.[gltf|glb|obj]>" << std::endl;
        return 1;
    }

    std::string path = args[1];

    auto rec = rerun::RecordingStream("rerun_example_asset3d_simple");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis
    rec.log("world/asset", rerun::Asset3D::from_file(path));
}
