// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/annotation_info.fbs".

#pragma once

#include "../result.hpp"
#include "rgba32.hpp"
#include "utf8.hpp"

#include <cstdint>
#include <memory>
#include <optional>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: Annotation info annotating a class id or key-point id.
    ///
    /// Color and label will be used to annotate entities/keypoints which reference the id.
    /// The id refers either to a class or key-point id
    struct AnnotationInfo {
        /// `datatypes::ClassId` or `datatypes::KeypointId` to which this annotation info belongs.
        uint16_t id;

        /// The label that will be shown in the UI.
        std::optional<rerun::datatypes::Utf8> label;

        /// The color that will be applied to the annotated entity.
        std::optional<rerun::datatypes::Rgba32> color;

      public: // START of extensions from annotation_info_ext.cpp:
        AnnotationInfo(
            uint16_t _id, std::optional<std::string> _label = std::nullopt,
            std::optional<datatypes::Rgba32> _color = std::nullopt
        )
            : id(_id), label(std::move(_label)), color(_color) {}

        AnnotationInfo(uint16_t _id, datatypes::Rgba32 _color)
            : id(_id), label(std::nullopt), color(_color) {}

        // END of extensions from annotation_info_ext.cpp, start of generated code:

      public:
        AnnotationInfo() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::AnnotationInfo> {
        static constexpr const char Name[] = "rerun.datatypes.AnnotationInfo";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::AnnotationInfo` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::AnnotationInfo* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const datatypes::AnnotationInfo* elements,
            size_t num_elements
        );
    };
} // namespace rerun
