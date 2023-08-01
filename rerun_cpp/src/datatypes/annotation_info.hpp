// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/annotation_info.fbs"

#pragma once

#include "../components/color.hpp"
#include "../components/label.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <optional>

namespace rr {
    namespace datatypes {
        /// Annotation info annotating a class id or key-point id.
        ///
        /// Color and label will be used to annotate entities/keypoints which reference the id.
        /// The id refers either to a class or key-point id
        struct AnnotationInfo {
            ///[`ClassId`] or [`KeypointId`] to which this annotation info belongs.
            uint16_t id;

            /// The label that will be shown in the UI.
            std::optional<rr::components::Label> label;

            /// The color that will be applied to the annotated entity.
            std::optional<rr::components::Color> color;

          public:
            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const AnnotationInfo* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rr
