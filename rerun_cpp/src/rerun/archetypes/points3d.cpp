// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/points3d.fbs".

#include "points3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char Points3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Points3DIndicator";
}

namespace rerun {

    Result<std::vector<SerializedComponentBatch>> AsComponents<archetypes::Points3D>::serialize(
        const archetypes::Points3D& archetype
    ) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(7);

        {
            const size_t size = archetype.positions.size();
            auto result = rerun::components::Position3D::to_data_cell(
                archetype.positions.data(),
                archetype.positions.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        if (archetype.radii.has_value()) {
            const size_t size = archetype.radii.value().size();
            auto result =
                rerun::components::Radius::to_data_cell(archetype.radii.value().data(), size);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        if (archetype.colors.has_value()) {
            const size_t size = archetype.colors.value().size();
            auto result =
                rerun::components::Color::to_data_cell(archetype.colors.value().data(), size);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        if (archetype.labels.has_value()) {
            const size_t size = archetype.labels.value().size();
            auto result =
                rerun::components::Text::to_data_cell(archetype.labels.value().data(), size);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        if (archetype.class_ids.has_value()) {
            const size_t size = archetype.class_ids.value().size();
            auto result =
                rerun::components::ClassId::to_data_cell(archetype.class_ids.value().data(), size);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        if (archetype.keypoint_ids.has_value()) {
            const size_t size = archetype.keypoint_ids.value().size();
            auto result = rerun::components::KeypointId::to_data_cell(
                archetype.keypoint_ids.value().data(),
                size
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        if (archetype.instance_keys.has_value()) {
            const size_t size = archetype.instance_keys.value().size();
            auto result = rerun::components::InstanceKey::to_data_cell(
                archetype.instance_keys.value().data(),
                size
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), size);
        }
        {
            auto indicator = Points3D::IndicatorComponent();
            auto result = Points3D::IndicatorComponent::to_data_cell(&indicator, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value), 1);
        }

        return cells;
    }
} // namespace rerun
