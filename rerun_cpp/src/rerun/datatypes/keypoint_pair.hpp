// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/keypoint_pair.fbs".

#pragma once

#include "../result.hpp"
#include "keypoint_id.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A connection between two `Keypoints`.
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

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const KeypointPair* elements, size_t num_elements
        );
    };
} // namespace rerun::datatypes
