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

        /// Optional name of the field within `Archetype` associated with this data.
        ///
        /// `None` if the data wasn't logged through an archetype.
        ///
        /// Example: `positions`.
        std::optional<std::string_view> archetype_field_name;

        /// Semantic name associated with this data.
        ///
        /// This is fully implied by `archetype_name` and `archetype_field`, but
        /// included for semantic convenience.
        ///
        /// Example: `rerun.components.Position3D`.
        std::string_view component_name;

        constexpr ComponentDescriptor(
            std::optional<std::string_view> archetype_name_,
            std::optional<std::string_view> archetype_field_name_, std::string_view component_name_
        )
            : archetype_name(archetype_name_),
              archetype_field_name(archetype_field_name_),
              component_name(component_name_) {}

        constexpr ComponentDescriptor(
            const char* archetype_name_, const char* archetype_field_name_,
            const char* component_name_
        )
            : archetype_name(archetype_name_),
              archetype_field_name(archetype_field_name_),
              component_name(component_name_) {}

        constexpr ComponentDescriptor(std::string_view component_name_)
            : component_name(component_name_) {}

        constexpr ComponentDescriptor(const char* component_name_)
            : component_name(component_name_) {}

        ComponentDescriptorHash hashed() const {
            std::size_t archetype_name_h =
                std::hash<std::optional<std::string_view>>{}(this->archetype_name);
            std::size_t component_name_h = std::hash<std::string_view>{}(this->component_name);
            std::size_t archetype_field_name_h =
                std::hash<std::optional<std::string_view>>{}(this->archetype_field_name);
            return archetype_name_h ^ component_name_h ^ archetype_field_name_h;
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

        /// Unconditionally sets `archetype_field_name` to the given one.
        ComponentDescriptor with_archetype_field_name(
            std::optional<std::string_view> archetype_field_name_
        ) const {
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_field_name = archetype_field_name_;
            return descriptor;
        }

        /// Unconditionally sets `archetype_field_name` to the given one.
        ComponentDescriptor with_archetype_field_name(const char* archetype_field_name_) const {
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_field_name = archetype_field_name_;
            return descriptor;
        }

        /// Sets `archetype_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype_name(std::optional<std::string_view> archetype_name_
        ) const {
            if (this->archetype_field_name.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_name = archetype_name_;
            return descriptor;
        }

        /// Sets `archetype_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype_name(const char* archetype_name_) const {
            if (this->archetype_field_name.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_name = archetype_name_;
            return descriptor;
        }

        /// Sets `archetype_field_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype_field_name(
            std::optional<std::string_view> archetype_field_name_
        ) const {
            if (this->archetype_field_name.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_field_name = archetype_field_name_;
            return descriptor;
        }

        /// Sets `archetype_field_name` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype_field_name(const char* archetype_field_name_) const {
            if (this->archetype_field_name.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype_field_name = archetype_field_name_;
            return descriptor;
        }
    };
} // namespace rerun
