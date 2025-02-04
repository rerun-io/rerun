// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_slice_selection.fbs".

#pragma once

#include "../../blueprint/components/tensor_dimension_index_slider.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../component_column.hpp"
#include "../../components/tensor_dimension_index_selection.hpp"
#include "../../components/tensor_height_dimension.hpp"
#include "../../components/tensor_width_dimension.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Specifies a 2D slice of a tensor.
    struct TensorSliceSelection {
        /// Which dimension to map to width.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        std::optional<ComponentBatch> width;

        /// Which dimension to map to height.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        std::optional<ComponentBatch> height;

        /// Selected indices for all other dimensions.
        ///
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        std::optional<ComponentBatch> indices;

        /// Any dimension listed here will have a slider for the index.
        ///
        /// Edits to the sliders will directly manipulate dimensions on the `indices` list.
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        /// If not specified, adds slides for any dimension in `indices`.
        std::optional<ComponentBatch> slider;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.TensorSliceSelectionIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] =
            "rerun.blueprint.archetypes.TensorSliceSelection";

        /// `ComponentDescriptor` for the `width` field.
        static constexpr auto Descriptor_width = ComponentDescriptor(
            ArchetypeName, "width",
            Loggable<rerun::components::TensorWidthDimension>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `height` field.
        static constexpr auto Descriptor_height = ComponentDescriptor(
            ArchetypeName, "height",
            Loggable<rerun::components::TensorHeightDimension>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `indices` field.
        static constexpr auto Descriptor_indices = ComponentDescriptor(
            ArchetypeName, "indices",
            Loggable<rerun::components::TensorDimensionIndexSelection>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `slider` field.
        static constexpr auto Descriptor_slider = ComponentDescriptor(
            ArchetypeName, "slider",
            Loggable<rerun::blueprint::components::TensorDimensionIndexSlider>::Descriptor
                .component_name
        );

      public:
        TensorSliceSelection() = default;
        TensorSliceSelection(TensorSliceSelection&& other) = default;
        TensorSliceSelection(const TensorSliceSelection& other) = default;
        TensorSliceSelection& operator=(const TensorSliceSelection& other) = default;
        TensorSliceSelection& operator=(TensorSliceSelection&& other) = default;

        /// Update only some specific fields of a `TensorSliceSelection`.
        static TensorSliceSelection update_fields() {
            return TensorSliceSelection();
        }

        /// Clear all the fields of a `TensorSliceSelection`.
        static TensorSliceSelection clear_fields();

        /// Which dimension to map to width.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        TensorSliceSelection with_width(const rerun::components::TensorWidthDimension& _width) && {
            width = ComponentBatch::from_loggable(_width, Descriptor_width).value_or_throw();
            return std::move(*this);
        }

        /// Which dimension to map to height.
        ///
        /// If not specified, the height will be determined automatically based on the name and index of the dimension.
        TensorSliceSelection with_height(const rerun::components::TensorHeightDimension& _height
        ) && {
            height = ComponentBatch::from_loggable(_height, Descriptor_height).value_or_throw();
            return std::move(*this);
        }

        /// Selected indices for all other dimensions.
        ///
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        TensorSliceSelection with_indices(
            const Collection<rerun::components::TensorDimensionIndexSelection>& _indices
        ) && {
            indices = ComponentBatch::from_loggable(_indices, Descriptor_indices).value_or_throw();
            return std::move(*this);
        }

        /// Any dimension listed here will have a slider for the index.
        ///
        /// Edits to the sliders will directly manipulate dimensions on the `indices` list.
        /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        /// If not specified, adds slides for any dimension in `indices`.
        TensorSliceSelection with_slider(
            const Collection<rerun::blueprint::components::TensorDimensionIndexSlider>& _slider
        ) && {
            slider = ComponentBatch::from_loggable(_slider, Descriptor_slider).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::TensorSliceSelection> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::TensorSliceSelection& archetype
        );
    };
} // namespace rerun
