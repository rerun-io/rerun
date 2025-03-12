// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/asset3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/albedo_factor.hpp"
#include "../components/blob.hpp"
#include "../components/media_type.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <filesystem>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A prepacked 3D asset (`.gltf`, `.glb`, `.obj`, `.stl`, etc.).
    ///
    /// See also `archetypes::Mesh3D`.
    ///
    /// If there are multiple `archetypes::InstancePoses3D` instances logged to the same entity as a mesh,
    /// an instance of the mesh will be drawn for each transform.
    ///
    /// ## Example
    ///
    /// ### Simple 3D asset
    /// ![image](https://static.rerun.io/asset3d_simple/af238578188d3fd0de3e330212120e2842a8ddb2/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <iostream>
    ///
    /// int main(int argc, char* argv[]) {
    ///     if (argc <2) {
    ///         std::cerr <<"Usage: " <<argv[0] <<" <path_to_asset.[gltf|glb|obj|stl]>" <<std::endl;
    ///         return 1;
    ///     }
    ///
    ///     const auto path = argv[1];
    ///
    ///     const auto rec = rerun::RecordingStream("rerun_example_asset3d");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log_static("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis
    ///     rec.log("world/asset", rerun::Asset3D::from_file_path(path).value_or_throw());
    /// }
    /// ```
    struct Asset3D {
        /// The asset's bytes.
        std::optional<ComponentBatch> blob;

        /// The Media Type of the asset.
        ///
        /// Supported values:
        /// * `model/gltf-binary`
        /// * `model/gltf+json`
        /// * `model/obj` (.mtl material files are not supported yet, references are silently ignored)
        /// * `model/stl`
        ///
        /// If omitted, the viewer will try to guess from the data blob.
        /// If it cannot guess, it won't be able to render the asset.
        std::optional<ComponentBatch> media_type;

        /// A color multiplier applied to the whole asset.
        ///
        /// For mesh who already have `albedo_factor` in materials,
        /// it will be overwritten by actual `albedo_factor` of `archetypes::Asset3D` (if specified).
        std::optional<ComponentBatch> albedo_factor;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Asset3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Asset3D";

        /// `ComponentDescriptor` for the `blob` field.
        static constexpr auto Descriptor_blob = ComponentDescriptor(
            ArchetypeName, "blob", Loggable<rerun::components::Blob>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `media_type` field.
        static constexpr auto Descriptor_media_type = ComponentDescriptor(
            ArchetypeName, "media_type",
            Loggable<rerun::components::MediaType>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `albedo_factor` field.
        static constexpr auto Descriptor_albedo_factor = ComponentDescriptor(
            ArchetypeName, "albedo_factor",
            Loggable<rerun::components::AlbedoFactor>::Descriptor.component_name
        );

      public: // START of extensions from asset3d_ext.cpp:
        /// Creates a new `Asset3D` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        ///
        /// \deprecated Use `from_file_path` instead.
        [[deprecated("Use `from_file_path` instead")]] static Result<Asset3D> from_file(
            const std::filesystem::path& path
        );

        /// Creates a new `Asset3D` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        static Result<Asset3D> from_file_path(const std::filesystem::path& path);

        /// Creates a new `Asset3D` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        ///
        /// \deprecated Use `from_file_contents` instead.
        [[deprecated("Use `from_file_contents` instead")]] static Asset3D from_bytes(
            rerun::Collection<uint8_t> bytes,
            std::optional<rerun::components::MediaType> media_type = {}
        ) {
            return from_file_contents(bytes, media_type);
        }

        /// Creates a new `Asset3D` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        static Asset3D from_file_contents(
            rerun::Collection<uint8_t> bytes,
            std::optional<rerun::components::MediaType> media_type = {}
        ) {
            Asset3D asset = Asset3D(std::move(bytes));
            // TODO(cmc): we could try and guess using magic bytes here, like rust does.
            if (media_type.has_value()) {
                return std::move(asset).with_media_type(media_type.value());
            }
            return asset;
        }

        // END of extensions from asset3d_ext.cpp, start of generated code:

      public:
        Asset3D() = default;
        Asset3D(Asset3D&& other) = default;
        Asset3D(const Asset3D& other) = default;
        Asset3D& operator=(const Asset3D& other) = default;
        Asset3D& operator=(Asset3D&& other) = default;

        explicit Asset3D(rerun::components::Blob _blob)
            : blob(ComponentBatch::from_loggable(std::move(_blob), Descriptor_blob).value_or_throw()
              ) {}

        /// Update only some specific fields of a `Asset3D`.
        static Asset3D update_fields() {
            return Asset3D();
        }

        /// Clear all the fields of a `Asset3D`.
        static Asset3D clear_fields();

        /// The asset's bytes.
        Asset3D with_blob(const rerun::components::Blob& _blob) && {
            blob = ComponentBatch::from_loggable(_blob, Descriptor_blob).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `blob` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_blob` should
        /// be used when logging a single row's worth of data.
        Asset3D with_many_blob(const Collection<rerun::components::Blob>& _blob) && {
            blob = ComponentBatch::from_loggable(_blob, Descriptor_blob).value_or_throw();
            return std::move(*this);
        }

        /// The Media Type of the asset.
        ///
        /// Supported values:
        /// * `model/gltf-binary`
        /// * `model/gltf+json`
        /// * `model/obj` (.mtl material files are not supported yet, references are silently ignored)
        /// * `model/stl`
        ///
        /// If omitted, the viewer will try to guess from the data blob.
        /// If it cannot guess, it won't be able to render the asset.
        Asset3D with_media_type(const rerun::components::MediaType& _media_type) && {
            media_type =
                ComponentBatch::from_loggable(_media_type, Descriptor_media_type).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `media_type` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_media_type` should
        /// be used when logging a single row's worth of data.
        Asset3D with_many_media_type(const Collection<rerun::components::MediaType>& _media_type
        ) && {
            media_type =
                ComponentBatch::from_loggable(_media_type, Descriptor_media_type).value_or_throw();
            return std::move(*this);
        }

        /// A color multiplier applied to the whole asset.
        ///
        /// For mesh who already have `albedo_factor` in materials,
        /// it will be overwritten by actual `albedo_factor` of `archetypes::Asset3D` (if specified).
        Asset3D with_albedo_factor(const rerun::components::AlbedoFactor& _albedo_factor) && {
            albedo_factor = ComponentBatch::from_loggable(_albedo_factor, Descriptor_albedo_factor)
                                .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `albedo_factor` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_albedo_factor` should
        /// be used when logging a single row's worth of data.
        Asset3D with_many_albedo_factor(
            const Collection<rerun::components::AlbedoFactor>& _albedo_factor
        ) && {
            albedo_factor = ComponentBatch::from_loggable(_albedo_factor, Descriptor_albedo_factor)
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

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::Asset3D> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(const archetypes::Asset3D& archetype);
    };
} // namespace rerun
