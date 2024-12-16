// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/boxes3d.fbs".

#include "../collection_adapter_builtins.hpp"
#include "boxes3d.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Boxes3D>::serialize(
        const archetypes::Boxes3D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(11);

        {
            auto result = ComponentBatch::from_loggable(
                archetype.half_sizes,
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "half_sizes",
                    "rerun.components.HalfSize3D"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.centers.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.centers.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "centers",
                    "rerun.components.PoseTranslation3D"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.rotation_axis_angles.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.rotation_axis_angles.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "rotation_axis_angles",
                    "rerun.components.PoseRotationAxisAngle"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.quaternions.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.quaternions.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "quaternions",
                    "rerun.components.PoseRotationQuat"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.colors.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.colors.value(),
                ComponentDescriptor("rerun.archetypes.Boxes3D", "colors", "rerun.components.Color")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.radii.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.radii.value(),
                ComponentDescriptor("rerun.archetypes.Boxes3D", "radii", "rerun.components.Radius")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.fill_mode.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.fill_mode.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "fill_mode",
                    "rerun.components.FillMode"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.labels.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.labels.value(),
                ComponentDescriptor("rerun.archetypes.Boxes3D", "labels", "rerun.components.Text")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.show_labels.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.show_labels.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "show_labels",
                    "rerun.components.ShowLabels"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.class_ids.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Boxes3D",
                    "class_ids",
                    "rerun.components.ClassId"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Boxes3D::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
