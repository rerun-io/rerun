#pragma once

#include <cstdint>
#include <optional>
#include <string_view>

namespace rerun {
    /// See `ComponentDescriptor::hashed`.
    using ComponentDescriptorHash = uint64_t;

    /// A `ComponentDescriptor` fully describes the semantics of a column of data.
    ///
    /// Every component at a given entity path is uniquely identified by the
    /// `component` field of the descriptor. The `archetype` and `component_type`
    /// fields provide additional information about the semantics of the data.
    struct ComponentDescriptor {
        /// Optional name of the `Archetype` associated with this data.
        ///
        /// `None` if the data wasn't logged through an archetype.
        ///
        /// Example: `rerun.archetypes.Points3D`.
        std::optional<std::string_view> archetype;

        /// Uniquely identifies of the component associated with this data.
        ///
        /// Example: `Points3D:positions`.
        std::string_view component;

        /// Optional type information for this component.
        ///
        /// Can be used to inform applications on how to interpret the data.
        ///
        /// Example: `rerun.components.Position3D`.
        std::optional<std::string_view> component_type;

        constexpr ComponentDescriptor(
            std::optional<std::string_view> archetype_, std::string_view component_,
            std::optional<std::string_view> component_type_
        )
            : archetype(archetype_), component(component_), component_type(component_type_) {}

        constexpr ComponentDescriptor(
            const char* archetype_, const char* component_, const char* component_type_
        )
            : archetype(archetype_), component(component_), component_type(component_type_) {}

        constexpr ComponentDescriptor(std::string_view component_) : component(component_) {}

        constexpr ComponentDescriptor(const char* component_) : component(component_) {}

        ComponentDescriptorHash hashed() const {
            std::size_t archetype_h = std::hash<std::optional<std::string_view>>{}(this->archetype);
            std::size_t component_type_h =
                std::hash<std::optional<std::string_view>>{}(this->component_type);
            std::size_t component_h = std::hash<std::string_view>{}(this->component);
            return archetype_h ^ component_type_h ^ component_h;
        }

        /// Unconditionally sets `archetype` to the given one.
        ComponentDescriptor with_archetype(std::optional<std::string_view> archetype_) const {
            ComponentDescriptor descriptor = *this;
            descriptor.archetype = archetype_;
            return descriptor;
        }

        /// Unconditionally sets `archetype` to the given one.
        ComponentDescriptor with_archetype(const char* archetype_) const {
            ComponentDescriptor descriptor = *this;
            descriptor.archetype = archetype_;
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

        /// Sets `archetype` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype(std::optional<std::string_view> archetype_) const {
            if (this->archetype.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype = archetype_;
            return descriptor;
        }

        /// Sets `archetype` to the given one iff it's not already set.
        ComponentDescriptor or_with_archetype(const char* archetype_) const {
            if (this->archetype.has_value()) {
                return *this;
            }
            ComponentDescriptor descriptor = *this;
            descriptor.archetype = archetype_;
            return descriptor;
        }

        /// Sets `component_type` to the given one iff it's not already set.
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
