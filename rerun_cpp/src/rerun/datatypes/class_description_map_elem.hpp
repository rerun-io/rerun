// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/class_description_map_elem.fbs".

#pragma once

#include "../result.hpp"
#include "class_description.hpp"
#include "class_id.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A helper type for mapping class IDs to class descriptions.
    ///
    /// This is internal to the `AnnotationContext` structure.
    struct ClassDescriptionMapElem {
        /// The key: the class ID.
        rerun::datatypes::ClassId class_id;

        /// The value: class name, color, etc.
        rerun::datatypes::ClassDescription class_description;

      public:
        // Extensions to generated type defined in 'class_description_map_elem_ext.cpp'

        ClassDescriptionMapElem(ClassDescription _class_description)
            : class_id(_class_description.info.id),
              class_description(std::move(_class_description)) {}

      public:
        ClassDescriptionMapElem() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::ClassDescriptionMapElem> {
        static constexpr const char Name[] = "rerun.datatypes.ClassDescriptionMapElem";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::ClassDescriptionMapElem` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::ClassDescriptionMapElem* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const datatypes::ClassDescriptionMapElem* elements,
            size_t num_elements
        );
    };
} // namespace rerun
