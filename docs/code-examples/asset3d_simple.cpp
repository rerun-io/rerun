// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

namespace rr = rerun;

int main(int argc, char* argv[]) {
    std::vector<std::string> args(argv, argv + argc);

    if (args.size() < 2) {
        std::cerr << "Usage: " << args[0] << " <path_to_asset.[gltf|glb]>" << std::endl;
        return 1;
    }

    std::string path = args[1];

    auto rec = rr::RecordingStream("rerun_example_asset3d_simple");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // TODO(#2816): some viewcoords would be nice here
    rec.log("asset", rr::Asset3D::from_file(path));
}
