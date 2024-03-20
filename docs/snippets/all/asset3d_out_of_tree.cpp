// Log a simple 3D asset with an out-of-tree transform which will not affect its children.

#include <exception>
#include <filesystem>

#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

int main(int argc, char** argv) {
    if (argc < 2) {
        throw std::runtime_error("Usage: <path_to_asset.[gltf|glb]>");
    }
    const auto path = argv[1];

    const auto rec = rerun::RecordingStream("rerun_example_asset3d_out_of_tree");
    rec.spawn().exit_on_failure();

    rec.log_timeless("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis

    rec.set_time_sequence("frame", 0);
    rec.log("world/asset", rerun::Asset3D::from_file(path).value_or_throw());
    // Those points will not be affected by their parent's out-of-tree transform!
    rec.log(
        "world/asset/points",
        rerun::Points3D(rerun::demo::grid3d<rerun::Position3D, float>(-10.0f, 10.0f, 10))
    );

    for (int64_t i = 1; i < 20; ++i) {
        rec.set_time_sequence("frame", i);

        // Modify the asset's out-of-tree transform: this will not affect its children (i.e. the points)!
        const auto translation =
            rerun::TranslationRotationScale3D({0.0, 0.0, static_cast<float>(i) - 10.0f});
        rec.log(
            "world/asset",
            rerun::Collection<rerun::OutOfTreeTransform3D>(rerun::OutOfTreeTransform3D(translation))
        );
    }
}
