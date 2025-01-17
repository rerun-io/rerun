// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/asset3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
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
    ///     rec.log("world/asset", rerun::Asset3D::from_file(path).value_or_throw());
    /// }
    /// ```
    struct Asset3D {
        /// The asset's bytes.
        rerun::components::Blob blob;

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
        std::optional<rerun::components::MediaType> media_type;

        /// A color multiplier applied to the whole asset.
        ///
        /// For mesh who already have `albedo_factor` in materials,
        /// it will be overwritten by actual `albedo_factor` of `archetypes::Asset3D` (if specified).
        std::optional<rerun::components::AlbedoFactor> albedo_factor;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Asset3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        static constexpr const char ArchetypeName[] = "rerun.archetypes.Asset3D";

      public: // START of extensions from asset3d_ext.cpp:
        /// Creates a new `Asset3D` from the file contents at `path`.
        ///
        /// The `MediaType` will be guessed from the file extension.
        ///
        /// If no `MediaType` can be guessed at the moment, the Rerun Viewer will try to guess one
        /// from the data at render-time. If it can't, rendering will fail with an error.
        static Result<Asset3D> from_file(const std::filesystem::path& path);

        /// Creates a new `Asset3D` from the given `bytes`.
        ///
        /// If no `MediaType` is specified, the Rerun Viewer will try to guess one from the data
        /// at render-time. If it can't, rendering will fail with an error.
        static Asset3D from_bytes(
            rerun::Collection<uint8_t> bytes,
            std::optional<rerun::components::MediaType> media_type = {}
        ) {
            // TODO(cmc): we could try and guess using magic bytes here, like rust does.
            Asset3D asset = Asset3D(std::move(bytes));
            asset.media_type = media_type;
            return asset;
        }

        // END of extensions from asset3d_ext.cpp, start of generated code:

      public:
        Asset3D() = default;
        Asset3D(Asset3D&& other) = default;
        Asset3D(const Asset3D& other) = default;
        Asset3D& operator=(const Asset3D& other) = default;
        Asset3D& operator=(Asset3D&& other) = default;

        explicit Asset3D(rerun::components::Blob _blob) : blob(std::move(_blob)) {}

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
        Asset3D with_media_type(rerun::components::MediaType _media_type) && {
            media_type = std::move(_media_type);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// A color multiplier applied to the whole asset.
        ///
        /// For mesh who already have `albedo_factor` in materials,
        /// it will be overwritten by actual `albedo_factor` of `archetypes::Asset3D` (if specified).
        Asset3D with_albedo_factor(rerun::components::AlbedoFactor _albedo_factor) && {
            albedo_factor = std::move(_albedo_factor);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
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
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::Asset3D& archetype);
    };
} // namespace rerun
