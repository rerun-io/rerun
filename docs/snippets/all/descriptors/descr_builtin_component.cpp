#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_builtin_component");
    rec.spawn().exit_on_failure();

    rec.log_static(
        "data",
        rerun::ComponentBatch::from_loggable(
            rerun::Position3D(1.0f, 2.0f, 3.0f),
            rerun::ComponentDescriptor(
                "user.CustomPoints3D",                            // archetype name
                "user.CustomPoints3D:points",                     // component
                rerun::Loggable<rerun::Position3D>::ComponentType // component type
            )
        )
    );

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
