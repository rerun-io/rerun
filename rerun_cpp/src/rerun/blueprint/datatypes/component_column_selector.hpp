// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/component_column_selector.fbs".

#pragma once

#include "../../datatypes/entity_path.hpp"
#include "../../datatypes/utf8.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::blueprint::datatypes {
    /// **Datatype**: Describe a component column to be selected in the dataframe view.
    struct ComponentColumnSelector {
        /// The entity path for this component.
        rerun::datatypes::EntityPath entity_path;

        /// The name of the component.
        rerun::datatypes::Utf8 component;

      public:
        ComponentColumnSelector() = default;
    };
} // namespace rerun::blueprint::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<blueprint::datatypes::ComponentColumnSelector> {
        static constexpr const char Name[] = "rerun.blueprint.datatypes.ComponentColumnSelector";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::blueprint:: datatypes::ComponentColumnSelector` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::datatypes::ComponentColumnSelector* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder,
            const blueprint::datatypes::ComponentColumnSelector* elements, size_t num_elements
        );
    };
} // namespace rerun
