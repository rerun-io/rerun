// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/components/viewer_recommendation_hash.fbs".

#pragma once

#include "../../datatypes/uint64.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

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
    /// \private
    template <>
    struct Loggable<blueprint::components::ViewerRecommendationHash> {
        using TypeFwd = rerun::datatypes::UInt64;
        static_assert(sizeof(TypeFwd) == sizeof(blueprint::components::ViewerRecommendationHash));
        static constexpr const char Name[] = "rerun.blueprint.components.ViewerRecommendationHash";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<TypeFwd>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::ViewerRecommendationHash` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::ViewerRecommendationHash* instances, size_t num_instances
        ) {
            return Loggable<TypeFwd>::to_arrow(
                reinterpret_cast<const TypeFwd*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
