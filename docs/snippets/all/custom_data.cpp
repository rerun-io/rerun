/// Demonstrates how to implement custom archetypes and components, and extend existing ones.

#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

/// A custom component type.
struct Confidence {
    float value;
};

/// A custom archetype that extends Rerun's builtin `rerun::Points3D` archetype with a custom component.
struct CustomPoints3D {
    static constexpr const char IndicatorName[] = "user.CustomPoints3DIndicator";
    using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorName>;

    rerun::Points3D points;
    // Using a rerun::Collection is not strictly necessary, you could also use an std::vector for example,
    // but useful for avoiding allocations since `rerun::Collection` can borrow data from other containers.
    std::optional<rerun::Collection<Confidence>> confidences;
};

template <>
struct rerun::AsComponents<CustomPoints3D> {
    static Result<std::vector<DataCell>> serialize(const CustomPoints3D& archetype) {
        auto cells = AsComponents<rerun::Points3D>::serialize(archetype.points).value_or_throw();

        // Add a custom indicator component.
        CustomPoints3D::IndicatorComponent indicator;
        cells.push_back(DataCell::from_loggable(indicator).value_or_throw());

        // Add custom confidence components if present.
        if (archetype.confidences) {
            cells.push_back(DataCell::from_loggable(*archetype.confidences).value_or_throw());
        }

        return cells;
    }
};

// ---

template <>
struct rerun::Loggable<Confidence> {
    static constexpr const char Name[] = "user.Confidence";

    static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
        return rerun::Loggable<rerun::Float32>::arrow_datatype();
    }

    // TODO(#4257) should take a rerun::Collection instead of pointer and size.
    static rerun::Result<std::shared_ptr<arrow::Array>> to_arrow(
        const Confidence* instances, size_t num_instances
    ) {
        return rerun::Loggable<rerun::Float32>::to_arrow(
            reinterpret_cast<const rerun::Float32*>(instances),
            num_instances
        );
    }
};

// ---

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_custom_data");
    rec.spawn().exit_on_failure();

    auto grid = rerun::demo::grid3d<rerun::Position3D, float>(-5.0f, 5.0f, 3);

    rec.log(
        "left/my_confident_point_cloud",
        CustomPoints3D{
            rerun::Points3D(grid),
            Confidence{42.0f},
        }
    );

    std::vector<Confidence> confidences;
    for (auto i = 0; i < 27; ++i) {
        confidences.emplace_back(Confidence{static_cast<float>(i)});
    }

    rec.log(
        "right/my_polarized_point_cloud",
        CustomPoints3D{
            rerun::Points3D(grid),
            confidences,
        }
    );
}
