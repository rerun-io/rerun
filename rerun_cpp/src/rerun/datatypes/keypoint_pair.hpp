// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/datatypes/keypoint_pair.fbs".

#pragma once

#include "../result.hpp"
#include "keypoint_id.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// A connection between two `Keypoints`.
        struct KeypointPair {
            rerun::datatypes::KeypointId keypoint0;

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

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const KeypointPair* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
