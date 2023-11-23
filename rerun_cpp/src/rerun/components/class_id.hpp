// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/class_id.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/class_id.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class DataType;
    class UInt16Type;
    using UInt16Builder = NumericBuilder<UInt16Type>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A 16-bit ID representing a type of semantic class.
    struct ClassId {
        rerun::datatypes::ClassId id;

      public:
        ClassId() = default;

        ClassId(rerun::datatypes::ClassId id_) : id(id_) {}

        ClassId& operator=(rerun::datatypes::ClassId id_) {
            id = id_;
            return *this;
        }

        ClassId(uint16_t id_) : id(id_) {}

        ClassId& operator=(uint16_t id_) {
            id = id_;
            return *this;
        }

        /// Cast to the underlying ClassId datatype
        operator rerun::datatypes::ClassId() const {
            return id;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::ClassId> {
        static constexpr const char Name[] = "rerun.components.ClassId";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt16Builder* builder, const components::ClassId* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::components::ClassId` components.
        static Result<rerun::DataCell> to_data_cell(
            const components::ClassId* instances, size_t num_instances
        );
    };
} // namespace rerun
