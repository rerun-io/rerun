// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/keypoint_id.fbs".

#pragma once

#include "../datatypes/keypoint_id.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class UInt16Type;
    using UInt16Builder = NumericBuilder<UInt16Type>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A 16-bit ID representing a type of semantic keypoint within a class.
    struct KeypointId {
        rerun::datatypes::KeypointId id;

      public:
        KeypointId() = default;

        KeypointId(rerun::datatypes::KeypointId id_) : id(id_) {}

        KeypointId& operator=(rerun::datatypes::KeypointId id_) {
            id = id_;
            return *this;
        }

        KeypointId(uint16_t id_) : id(id_) {}

        KeypointId& operator=(uint16_t id_) {
            id = id_;
            return *this;
        }

        /// Cast to the underlying KeypointId datatype
        operator rerun::datatypes::KeypointId() const {
            return id;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::KeypointId) == sizeof(components::KeypointId));

    /// \private
    template <>
    struct Loggable<components::KeypointId> {
        static constexpr const char Name[] = "rerun.components.KeypointId";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::KeypointId>::arrow_datatype();
        }

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt16Builder* builder, const components::KeypointId* elements,
            size_t num_elements
        ) {
            return Loggable<rerun::datatypes::KeypointId>::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::KeypointId*>(elements),
                num_elements
            );
        }

        /// Serializes an array of `rerun::components::KeypointId` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::KeypointId* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::KeypointId>::to_arrow(
                reinterpret_cast<const rerun::datatypes::KeypointId*>(instances),
                num_instances
            );
        }
    };
} // namespace rerun
