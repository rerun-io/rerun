// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/out_of_tree_transform3d.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/transform3d.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: An out-of-tree affine transform between two 3D spaces, represented in a given direction.
    ///
    /// "Out-of-tree" means that the transform only affects its own entity: children don't inherit from it.
    struct OutOfTreeTransform3D {
        /// Representation of the transform.
        rerun::datatypes::Transform3D repr;

      public:
        OutOfTreeTransform3D() = default;

        OutOfTreeTransform3D(rerun::datatypes::Transform3D repr_) : repr(repr_) {}

        OutOfTreeTransform3D& operator=(rerun::datatypes::Transform3D repr_) {
            repr = repr_;
            return *this;
        }

        /// Cast to the underlying Transform3D datatype
        operator rerun::datatypes::Transform3D() const {
            return repr;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::OutOfTreeTransform3D> {
        static constexpr const char Name[] = "rerun.components.OutOfTreeTransform3D";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::DenseUnionBuilder* builder, const components::OutOfTreeTransform3D* elements,
            size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::components::OutOfTreeTransform3D` components.
        static Result<rerun::DataCell> to_data_cell(
            const components::OutOfTreeTransform3D* instances, size_t num_instances
        );
    };
} // namespace rerun
