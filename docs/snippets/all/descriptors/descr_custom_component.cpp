#include <rerun.hpp>

struct CustomPosition3D {
    rerun::components::Position3D position;
};

template <>
struct rerun::Loggable<CustomPosition3D> {
    static constexpr const ComponentDescriptor Descriptor = ComponentDescriptor(
        "user.CustomArchetype", "user.CustomArchetypeField", "user.CustomPosition3D"
    );

    static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
        return rerun::Loggable<rerun::components::Position3D>::arrow_datatype();
    }

    // TODO(#4257) should take a rerun::Collection instead of pointer and size.
    static rerun::Result<std::shared_ptr<arrow::Array>> to_arrow(
        const CustomPosition3D* instances, size_t num_instances
    ) {
        return rerun::Loggable<rerun::components::Position3D>::to_arrow(
            reinterpret_cast<const rerun::components::Position3D*>(instances),
            num_instances
        );
    }
};

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_custom_component");
    rec.spawn().exit_on_failure();

    CustomPosition3D positions[1] = {rerun::components::Position3D{1.0f, 2.0f, 3.0f}};
    rec.log_static("data", positions);

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
