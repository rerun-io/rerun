// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/leaf_transforms3d.fbs".

#include "leaf_transforms3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::LeafTransforms3D>::serialize(
        const archetypes::LeafTransforms3D& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(6);

        if (archetype.translation.has_value()) {
            auto result = DataCell::from_loggable(archetype.translation.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.rotation_axis_angle.has_value()) {
            auto result = DataCell::from_loggable(archetype.rotation_axis_angle.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.quaternion.has_value()) {
            auto result = DataCell::from_loggable(archetype.quaternion.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.scale.has_value()) {
            auto result = DataCell::from_loggable(archetype.scale.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.mat3x3.has_value()) {
            auto result = DataCell::from_loggable(archetype.mat3x3.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = LeafTransforms3D::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
