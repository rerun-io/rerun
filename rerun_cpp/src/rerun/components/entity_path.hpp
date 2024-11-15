// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/entity_path.fbs".

#pragma once

#include "../datatypes/entity_path.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

namespace rerun::components {
    /// **Component**: A path to an entity, usually to reference some data that is part of the target entity.
    struct EntityPath {
        rerun::datatypes::EntityPath value;

      public: // START of extensions from entity_path_ext.cpp:
        EntityPath(std::string_view path_) : value(std::string(path_)) {}

        EntityPath(const char* path_) : value(std::string(path_)) {}

        // END of extensions from entity_path_ext.cpp, start of generated code:

      public:
        EntityPath() = default;

        EntityPath(rerun::datatypes::EntityPath value_) : value(std::move(value_)) {}

        EntityPath& operator=(rerun::datatypes::EntityPath value_) {
            value = std::move(value_);
            return *this;
        }

        EntityPath(std::string path_) : value(std::move(path_)) {}

        EntityPath& operator=(std::string path_) {
            value = std::move(path_);
            return *this;
        }

        /// Cast to the underlying EntityPath datatype
        operator rerun::datatypes::EntityPath() const {
            return value;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::EntityPath) == sizeof(components::EntityPath));

    /// \private
    template <>
    struct Loggable<components::EntityPath> {
        static constexpr const char Name[] = "rerun.components.EntityPath";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::EntityPath>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::EntityPath` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::EntityPath* instances, size_t num_instances
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
                    &instances->value,
                    num_instances
                );
            }
        }
    };
} // namespace rerun
