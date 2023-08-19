// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/class_description.fbs"

#pragma once

#include "../result.hpp"
#include "annotation_info.hpp"
#include "keypoint_pair.hpp"

#include <cstdint>
#include <vector>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        /// The description of a semantic Class.
        ///
        /// If an entity is annotated with a corresponding `ClassId`, rerun will use
        /// the attached `AnnotationInfo` to derive labels and colors.
        ///
        /// Keypoints within an annotation class can similarly be annotated with a
        ///`KeypointId` in which case we should defer to the label and color for the
        ///`AnnotationInfo` specifically associated with the Keypoint.
        ///
        /// Keypoints within the class can also be decorated with skeletal edges.
        /// Keypoint-connections are pairs of `KeypointId`s. If an edge is
        /// defined, and both keypoints exist within the instance of the class, then the
        /// keypoints should be connected with an edge. The edge should be labeled and
        /// colored as described by the class's `AnnotationInfo`.
        ///
        /// The default `info` is an `id=0` with no label or color.
        struct ClassDescription {
            /// The `AnnotationInfo` for the class.
            rerun::datatypes::AnnotationInfo info;

            /// The `AnnotationInfo` for all of the keypoints.
            std::vector<rerun::datatypes::AnnotationInfo> keypoint_annotations;

            /// The connections between keypoints.
            std::vector<rerun::datatypes::KeypointPair> keypoint_connections;

          public:
            // Extensions to generated type defined in 'class_description_ext.cpp'

            ClassDescription(
                AnnotationInfo _info, std::vector<AnnotationInfo> _keypoint_annotations = {},
                std::vector<KeypointPair> _keypoint_connections = {}
            )
                : info(std::move(_info)),
                  keypoint_annotations(std::move(_keypoint_annotations)),
                  keypoint_connections(std::move(_keypoint_connections)) {}

          public:
            ClassDescription() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const ClassDescription* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
