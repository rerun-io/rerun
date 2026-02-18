#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_builtin_archetype");
    rec.spawn().exit_on_failure();

    rec.log_static("data", rerun::Points3D({{1.0f, 2.0f, 3.0f}}).with_radii({0.3f, 0.2f, 0.1f}));

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
