#pragma once

#include <optional>
#include <string_view>

// TODO: to_string maybe?

namespace rerun {
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

        // TODO: {entity_path}@{archetype_name}:{component_name}#{archetype_field_name}

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

        // TODO: override helpers?
    };
} // namespace rerun
