// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/material.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/material.hpp"
#include "../datatypes/rgba32.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <optional>

namespace arrow {
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: Material properties of a mesh.
    struct Material {
        rerun::datatypes::Material material;

      public:
        // Extensions to generated type defined in 'material_ext.cpp'

        static Material from_albedo_factor(rerun::datatypes::Rgba32 color) {
            return Material(color);
        }

      public:
        Material() = default;

        Material(rerun::datatypes::Material material_) : material(material_) {}

        Material& operator=(rerun::datatypes::Material material_) {
            material = material_;
            return *this;
        }

        Material(std::optional<rerun::datatypes::Rgba32> albedo_factor_)
            : material(albedo_factor_) {}

        Material& operator=(std::optional<rerun::datatypes::Rgba32> albedo_factor_) {
            material = albedo_factor_;
            return *this;
        }

        /// Cast to the underlying Material datatype
        operator rerun::datatypes::Material() const {
            return material;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::Material> {
        static constexpr const char Name[] = "rerun.components.Material";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const components::Material* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::components::Material` components.
        static Result<rerun::DataCell> to_data_cell(
            const components::Material* instances, size_t num_instances
        );
    };
} // namespace rerun
