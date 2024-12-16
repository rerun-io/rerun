// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/transform3d.fbs".

#include "../collection_adapter_builtins.hpp"
#include "transform3d.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Transform3D>::serialize(
        const archetypes::Transform3D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(8);

        {
            auto result = ComponentBatch::from_loggable(
                archetype.translation,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "translation",
                    "rerun.components.Translation3D"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch::from_loggable(
                archetype.rotation_axis_angle,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "rotation_axis_angle",
                    "rerun.components.RotationAxisAngle"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch::from_loggable(
                archetype.quaternion,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "quaternion",
                    "rerun.components.RotationQuat"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch::from_loggable(
                archetype.scale,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "scale",
                    "rerun.components.Scale3D"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch::from_loggable(
                archetype.mat3x3,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "mat3x3",
                    "rerun.components.TransformMat3x3"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch::from_loggable(
                archetype.relation,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "relation",
                    "rerun.components.TransformRelation"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch::from_loggable(
                archetype.axis_length,
                ComponentDescriptor(
                    "rerun.archetypes.Transform3D",
                    "axis_length",
                    "rerun.components.AxisLength"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Transform3D::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
