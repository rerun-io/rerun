#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_builtin_component");
    rec.spawn().exit_on_failure();

    rec.log_static(
        "data",
        rerun::ComponentBatch::from_loggable(
            rerun::Position3D(1.0f, 2.0f, 3.0f),
            rerun::ComponentDescriptor(
                "user.CustomPoints3D", // archetype name
                "points",              // archetype field name
                // TODO(#6889): Clean up in follow up PR
                rerun::Loggable<rerun::Position3D>::Descriptor.component_name
            )
        )
    );

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
