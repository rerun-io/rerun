// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/rotation3d.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/rotation3d.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A 3D rotation, represented either by a quaternion or a rotation around axis.
    struct Rotation3D {
        /// Representation of the rotation.
        rerun::datatypes::Rotation3D repr;

        /// Name of the component, used for serialization.
        static const char NAME[];

      public:
        // Extensions to generated type defined in 'rotation3d_ext.cpp'

        static const Rotation3D IDENTITY;

        /// Construct Rotation3d from Quaternion.
        Rotation3D(datatypes::Quaternion quaternion) : repr{quaternion} {}

        /// Construct Rotation3d from axis-angle
        Rotation3D(datatypes::RotationAxisAngle axis_angle) : repr{axis_angle} {}

      public:
        Rotation3D() = default;

        Rotation3D(rerun::datatypes::Rotation3D repr_) : repr(repr_) {}

        Rotation3D& operator=(rerun::datatypes::Rotation3D repr_) {
            repr = repr_;
            return *this;
        }

        /// Cast to the underlying Rotation3D datatype
        operator rerun::datatypes::Rotation3D() const {
            return repr;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Creates a new array builder with an array of this type.
        static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::DenseUnionBuilder* builder, const Rotation3D* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of Rotation3D components.
        static Result<rerun::DataCell> to_data_cell(
            const Rotation3D* instances, size_t num_instances
        );
    };
} // namespace rerun::components
