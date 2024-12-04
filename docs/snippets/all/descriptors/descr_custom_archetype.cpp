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
    static constexpr const char IndicatorComponentName[] = "user.CustomPoints3DIndicator";
    using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

    rerun::Collection<CustomPosition3D> positions;
    std::optional<rerun::Collection<rerun::Color>> colors;
};

template <>
struct rerun::AsComponents<CustomPoints3D> {
    static Result<std::vector<ComponentBatch>> serialize(const CustomPoints3D& archetype) {
        std::vector<rerun::ComponentBatch> batches;

        CustomPoints3D::IndicatorComponent indicator;
        batches.push_back(ComponentBatch::from_loggable(indicator).value_or_throw());

        // TODO: with_methods would be nice
        auto positions_descr = rerun::ComponentDescriptor(
            "user.CustomArchetype",
            "positions",
            Loggable<CustomPosition3D>::Descriptor.component_name
        );
        batches.push_back(
            ComponentBatch::from_loggable(archetype.positions, positions_descr).value_or_throw()
        );

        if (archetype.colors) {
            // TODO: with_methods would be nice
            auto colors_descr = rerun::ComponentDescriptor(
                "user.CustomArchetype",
                "colors",
                Loggable<rerun::Color>::Descriptor.component_name
            );
            batches.push_back(
                ComponentBatch::from_loggable(archetype.colors, colors_descr).value_or_throw()
            );
        }

        return batches;
    }
};

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_descriptors_custom_archetype");
    rec.spawn().exit_on_failure();

    CustomPosition3D positions[1] = {rerun::components::Position3D{1.0f, 2.0f, 3.0f}};
    rerun::Color colors[1] = {rerun::Color(0xFF00FFFF)};

    rec.log_static("data", CustomPoints3D{positions, colors});

    // The tags are indirectly checked by the Rust version (have a look over there for more info).
}
