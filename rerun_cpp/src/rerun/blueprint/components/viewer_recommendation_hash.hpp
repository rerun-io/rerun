// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/viewer_recommendation_hash.fbs".

#pragma once

#include "../../datatypes/uint64.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class UInt64Type;
    using UInt64Builder = NumericBuilder<UInt64Type>;
} // namespace arrow

namespace rerun::blueprint::components {
    /// **Component**: Hash of a viewer recommendation.
    ///
    /// The formation of this hash is considered an internal implementation detail of the viewer.
    struct ViewerRecommendationHash {
        rerun::datatypes::UInt64 value;

      public:
        ViewerRecommendationHash() = default;

        ViewerRecommendationHash(rerun::datatypes::UInt64 value_) : value(value_) {}

        ViewerRecommendationHash& operator=(rerun::datatypes::UInt64 value_) {
            value = value_;
            return *this;
        }

        ViewerRecommendationHash(uint64_t value_) : value(value_) {}

        ViewerRecommendationHash& operator=(uint64_t value_) {
            value = value_;
            return *this;
        }

        /// Cast to the underlying UInt64 datatype
        operator rerun::datatypes::UInt64() const {
            return value;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::UInt64) == sizeof(blueprint::components::ViewerRecommendationHash)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::ViewerRecommendationHash> {
        static constexpr const char Name[] = "rerun.blueprint.components.ViewerRecommendationHash";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::UInt64>::arrow_datatype();
        }

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt64Builder* builder,
            const blueprint::components::ViewerRecommendationHash* elements, size_t num_elements
        ) {
            return Loggable<rerun::datatypes::UInt64>::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::UInt64*>(elements),
                num_elements
            );
        }

        /// Serializes an array of `rerun::blueprint:: components::ViewerRecommendationHash` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::ViewerRecommendationHash* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::UInt64>::to_arrow(
                reinterpret_cast<const rerun::datatypes::UInt64*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
