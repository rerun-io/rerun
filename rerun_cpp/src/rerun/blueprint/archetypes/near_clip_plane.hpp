// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/near_clip_plane.fbs".

#pragma once

#include "../../blueprint/components/near_clip_plane.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../component_column.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Controls the distance to the near clip plane in 3D scene units.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct NearClipPlane {
        /// Controls the distance to the near clip plane in 3D scene units.
        ///
        /// Content closer than this distance will not be visible.
        std::optional<ComponentBatch> near_clip_plane;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.NearClipPlaneIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.NearClipPlane";

        /// `ComponentDescriptor` for the `near_clip_plane` field.
        static constexpr auto Descriptor_near_clip_plane = ComponentDescriptor(
            ArchetypeName, "near_clip_plane",
            Loggable<rerun::blueprint::components::NearClipPlane>::Descriptor.component_name
        );

      public:
        NearClipPlane() = default;
        NearClipPlane(NearClipPlane&& other) = default;
        NearClipPlane(const NearClipPlane& other) = default;
        NearClipPlane& operator=(const NearClipPlane& other) = default;
        NearClipPlane& operator=(NearClipPlane&& other) = default;

        explicit NearClipPlane(rerun::blueprint::components::NearClipPlane _near_clip_plane)
            : near_clip_plane(ComponentBatch::from_loggable(
                                  std::move(_near_clip_plane), Descriptor_near_clip_plane
              )
                                  .value_or_throw()) {}

        /// Update only some specific fields of a `NearClipPlane`.
        static NearClipPlane update_fields() {
            return NearClipPlane();
        }

        /// Clear all the fields of a `NearClipPlane`.
        static NearClipPlane clear_fields();

        /// Controls the distance to the near clip plane in 3D scene units.
        ///
        /// Content closer than this distance will not be visible.
        NearClipPlane with_near_clip_plane(
            const rerun::blueprint::components::NearClipPlane& _near_clip_plane
        ) && {
            near_clip_plane =
                ComponentBatch::from_loggable(_near_clip_plane, Descriptor_near_clip_plane)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentBatch::partitioned`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::NearClipPlane> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::NearClipPlane& archetype
        );
    };
} // namespace rerun
