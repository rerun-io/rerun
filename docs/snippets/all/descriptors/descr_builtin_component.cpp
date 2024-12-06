#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_builtin_component");
    rec.spawn().exit_on_failure();

    rerun::Position3D positions[1] = {{1.0f, 2.0f, 3.0f}};
    rec.log_static("data", positions);

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
