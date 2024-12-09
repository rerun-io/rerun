// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/included_content.fbs".

#pragma once

#include "../../component_descriptor.hpp"
#include "../../datatypes/entity_path.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::blueprint::components {
    /// **Component**: All the contents in the container.
    struct IncludedContent {
        /// List of the contents by `datatypes::EntityPath`.
        ///
        /// This must be a path in the blueprint store.
        /// Typically structure as `<blueprint_registry>/<uuid>`.
        rerun::datatypes::EntityPath contents;

      public:
        IncludedContent() = default;

        IncludedContent(rerun::datatypes::EntityPath contents_) : contents(std::move(contents_)) {}

        IncludedContent& operator=(rerun::datatypes::EntityPath contents_) {
            contents = std::move(contents_);
            return *this;
        }

        IncludedContent(std::string path_) : contents(std::move(path_)) {}

        IncludedContent& operator=(std::string path_) {
            contents = std::move(path_);
            return *this;
        }

        /// Cast to the underlying EntityPath datatype
        operator rerun::datatypes::EntityPath() const {
            return contents;
        }
    };
} // namespace rerun::blueprint::components

namespace rerun {
    static_assert(
        sizeof(rerun::datatypes::EntityPath) == sizeof(blueprint::components::IncludedContent)
    );

    /// \private
    template <>
    struct Loggable<blueprint::components::IncludedContent> {
        static constexpr ComponentDescriptor Descriptor =
            "rerun.blueprint.components.IncludedContent";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::EntityPath>::arrow_datatype();
        }

        /// Serializes an array of `rerun::blueprint:: components::IncludedContent` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const blueprint::components::IncludedContent* instances, size_t num_instances
        ) {
            if (num_instances == 0) {
                return Loggable<rerun::datatypes::EntityPath>::to_arrow(nullptr, 0);
            } else if (instances == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array instances is null when num_elements> 0."
                );
            } else {
                return Loggable<rerun::datatypes::EntityPath>::to_arrow(
                    &instances->contents,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
