#pragma once

#include <cstdint>
#include <optional>
#include <string_view>

namespace rerun {
    /// See `ComponentDescriptor::hashed`.
    using ComponentDescriptorHash = uint64_t;

    /// A `ComponentDescriptor` fully describes the semantics of a column of data.
    ///
    /// Every component is uniquely identified by its `ComponentDescriptor`.
    struct ComponentDescriptor {
        /// Optional name of the `Archetype` associated with this data.
        ///
        /// `None` if the data wasn't logged through an archetype.
        ///
        /// Example: `rerun.archetypes.Points3D`.
        std::optional<std::string_view> archetype_name;

        /// Name of the field within `Archetype` associated with this data.
        ///
        /// Example: `positions`.
        std::string_view archetype_field_name;

        /// Optional semantic name associated with this data.
        ///
        /// This is fully implied by `archetype_name` and `archetype_field`, but
        /// included for semantic convenience.
        //
        /// `None` if the data wasn't logged through an archetype.
        ///
        /// Example: `rerun.components.Position3D`.
        std::optional<std::string_view> component_type;

        constexpr ComponentDescriptor(
            std::optional<std::string_view> archetype_name_, std::string_view archetype_field_name_,
            std::optional<std::string_view> component_type_
        )
            : archetype_name(archetype_name_),
              archetype_field_name(archetype_field_name_),
              component_type(component_type_) {}

        constexpr ComponentDescriptor(
            const char* archetype_name_, const char* archetype_field_name_,
            const char* component_type_
        )
            : archetype_name(archetype_name_),
              archetype_field_name(archetype_field_name_),
              component_type(component_type_) {}

        constexpr ComponentDescriptor(std::string_view archetype_field_name_)
            : archetype_field_name(archetype_field_name_) {}

        constexpr ComponentDescriptor(const char* archetype_field_name_)
            : archetype_field_name(archetype_field_name_) {}

        ComponentDescriptorHash hashed() const {
            std::size_t archetype_name_h =
                std::hash<std::optional<std::string_view>>{}(this->archetype_name);
            std::size_t component_type_h =
                std::hash<std::optional<std::string_view>>{}(this->component_type);
            std::size_t archetype_field_name_h =
                std::hash<std::string_view>{}(this->archetype_field_name);
            return archetype_name_h ^ component_type_h ^ archetype_field_name_h;
        }

        /// Unconditionally sets `archetype_name` to the given one.
        ComponentDescriptor with_archetype_name(std::optional<std::string_view> archetype_name_
        ) const {
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_name = archetype_name_;
            return descriptor;
        }

        /// Unconditionally sets `archetype_name` to the given one.
        ComponentDescriptor with_archetype_name(const char* archetype_name_) const {
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_name = archetype_name_;
            return descriptor;
        }

        /// Unconditionally sets `component_type` to the given one.
        ComponentDescriptor with_component_type(std::optional<std::string_view> component_type_
        ) const {
            ComponentDescriptor descriptor = *this;
            descriptor.component_type = component_type_;
            return descriptor;
        }

        /// Unconditionally sets `component_type` to the given one.
        ComponentDescriptor with_component_type(const char* component_type_) const {
            ComponentDescriptor descriptor = *this;
            descriptor.component_type = component_type_;
            return descriptor;
        }

        /// Sets `archetype_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype_name(std::optional<std::string_view> archetype_name_
        ) const {
            if (this->archetype_name.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_name = archetype_name_;
            return descriptor;
        }

        /// Sets `archetype_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype_name(const char* archetype_name_) const {
            if (this->archetype_name.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_name = archetype_name_;
            return descriptor;
        }

        /// Sets `archetype_field_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_component_type(std::optional<std::string_view> component_type_
        ) const {
            if (this->component_type.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.component_type = component_type_;
            return descriptor;
        }

        /// Sets `component_type` to the given one iff it's not already set.
        ComponentDescriptor or_with_component_type(const char* component_type_) const {
            if (this->component_type.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.component_type = component_type_;
            return descriptor;
        }
    };
} // namespace rerun
