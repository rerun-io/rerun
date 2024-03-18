// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/mesh_properties.fbs".

#pragma once

#include "../collection.hpp"
#include "../datatypes/mesh_properties.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <utility>

namespace rerun::components {
    /// **Component**: Optional triangle indices for a mesh.
    struct MeshProperties {
        rerun::datatypes::MeshProperties props;

      public:
        // Extensions to generated type defined in 'mesh_properties_ext.cpp'

        static MeshProperties from_triangle_indices(Collection<uint32_t> indices) {
            return MeshProperties(std::move(indices));
        }

      public:
        MeshProperties() = default;

        MeshProperties(rerun::datatypes::MeshProperties props_) : props(std::move(props_)) {}

        MeshProperties& operator=(rerun::datatypes::MeshProperties props_) {
            props = std::move(props_);
            return *this;
        }

        MeshProperties(std::optional<rerun::Collection<uint32_t>> indices_)
            : props(std::move(indices_)) {}

        MeshProperties& operator=(std::optional<rerun::Collection<uint32_t>> indices_) {
            props = std::move(indices_);
            return *this;
        }

        /// Cast to the underlying MeshProperties datatype
        operator rerun::datatypes::MeshProperties() const {
            return props;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::MeshProperties) == sizeof(components::MeshProperties));

    /// \private
    template <>
    struct Loggable<components::MeshProperties> {
        static constexpr const char Name[] = "rerun.components.MeshProperties";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::MeshProperties>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::MeshProperties` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::MeshProperties* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::MeshProperties>::to_arrow(
                &instances->props,
                num_instances
            );
        }
    };
} // namespace rerun
