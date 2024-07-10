// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/keypoint_pair.fbs".

#pragma once

#include "../result.hpp"
#include "keypoint_id.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A connection between two `datatypes::KeypointId`s.
    struct KeypointPair {
        /// The first point of the pair.
        rerun::datatypes::KeypointId keypoint0;

        /// The second point of the pair.
        rerun::datatypes::KeypointId keypoint1;

      public:
        // Extensions to generated type defined in 'keypoint_pair_ext.cpp'

        KeypointPair(uint16_t _keypoint0, uint16_t _keypoint1)
            : keypoint0(_keypoint0), keypoint1(_keypoint1) {}

        KeypointPair(std::pair<uint16_t, uint16_t> pair)
            : keypoint0(pair.first), keypoint1(pair.second) {}

      public:
        KeypointPair() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::KeypointPair> {
        static constexpr const char Name[] = "rerun.datatypes.KeypointPair";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::KeypointPair` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::KeypointPair* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const datatypes::KeypointPair* elements,
            size_t num_elements
        );
    };
} // namespace rerun
