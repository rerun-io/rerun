#include <rerun.hpp>
#include <vector>

struct CustomPosition3D {
    rerun::components::Position3D position;
};

template <>
struct rerun::Loggable<CustomPosition3D> {
    static constexpr ComponentDescriptor Descriptor = "user.CustomPosition3D";

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

/// A custom archetype that extends Rerun's builtin `rerun::Points3D` archetype with a custom component.
struct CustomPoints3D {
    rerun::Collection<CustomPosition3D> positions;
    std::optional<rerun::Collection<rerun::Color>> colors;
};

template <>
struct rerun::AsComponents<CustomPoints3D> {
    static Result<rerun::Collection<ComponentBatch>> as_batches(const CustomPoints3D& archetype) {
        std::vector<rerun::ComponentBatch> batches;

        auto positions_descr = rerun::ComponentDescriptor(
            "user.CustomPoints3D",
            "user.CustomPoints3D:custom_positions",
            "user.CustomPosition3D"
        );
        batches.push_back(
            ComponentBatch::from_loggable(archetype.positions, positions_descr).value_or_throw()
        );

        if (archetype.colors) {
            auto colors_descr =
                rerun::ComponentDescriptor("user.CustomPoints3D:colors")
                    .with_archetype("user.CustomPoints3D")
                    .with_component_type(rerun::Loggable<rerun::components::Color>::ComponentType);
            batches.push_back(
                ComponentBatch::from_loggable(archetype.colors, colors_descr).value_or_throw()
            );
        }

        return rerun::take_ownership(std::move(batches));
    }
};

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_custom_archetype");
    rec.spawn().exit_on_failure();

    rec.log_static(
        "data",
        CustomPoints3D{CustomPosition3D{{1.0f, 2.0f, 3.0f}}, rerun::Color(0xFF00FFFF)}
    );

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
