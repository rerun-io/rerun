// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/transform_relation.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class Array;
    class DataType;
    class UInt8Type;
    using UInt8Builder = NumericBuilder<UInt8Type>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: Specifies relation a spatial transform describes.
    enum class TransformRelation : uint8_t {

        /// The transform describes how to transform into the parent entity's space.
        ///
        /// E.g. a translation of (0, 1, 0) with this `components::TransformRelation` logged at `parent/child` means
        /// that from the point of view of `parent`, `parent/child` is translated 1 unit along `parent`'s Y axis.
        /// From perspective of `parent/child`, the `parent` entity is translated -1 unit along `parent/child`'s Y axis.
        ParentFromChild = 1,

        /// The transform describes how to transform into the child entity's space.
        ///
        /// E.g. a translation of (0, 1, 0) with this `components::TransformRelation` logged at `parent/child` means
        /// that from the point of view of `parent`, `parent/child` is translated -1 unit along `parent`'s Y axis.
        /// From perspective of `parent/child`, the `parent` entity is translated 1 unit along `parent/child`'s Y axis.
        ChildFromParent = 2,
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::TransformRelation> {
        static constexpr const char Name[] = "rerun.components.TransformRelation";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::TransformRelation` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::TransformRelation* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt8Builder* builder, const components::TransformRelation* elements,
            size_t num_elements
        );
    };
} // namespace rerun
