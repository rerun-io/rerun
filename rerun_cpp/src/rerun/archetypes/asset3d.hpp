// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/asset3d.fbs".

#pragma once

#include "../arrow.hpp"
#include "../component_batch.hpp"
#include "../components/blob.hpp"
#include "../components/media_type.hpp"
#include "../components/out_of_tree_transform3d.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <algorithm>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: A prepacked 3D asset (`.gltf`, `.glb`, `.obj`, etc.).
        ///
        /// ## Example
        ///
        /// ### Simple 3D asset
        /// ```cpp,ignore
        /// // Log a batch of 3D arrows.
        ///
        /// #include <rerun.hpp>
        ///
        /// #include <filesystem>
        /// #include <iostream>
        /// #include <string>
        /// #include <vector>
        ///
        /// namespace rr = rerun;
        ///
        /// int main(int argc, char* argv[]) {
        ///     std::vector<std::string> args(argv, argv + argc);
        ///
        ///     if (args.size() <2) {
        ///         std::cerr <<"Usage: " <<args[0] <<" <path_to_asset.[gltf|glb|obj]>" <<std::endl;
        ///         return 1;
        ///     }
        ///
        ///     std::string path = args[1];
        ///
        ///     auto rec = rr::RecordingStream("rerun_example_asset3d_simple");
        ///     rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///     rec.log("world", rr::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis
        ///     rec.log("world/asset", rr::Asset3D::from_file(path));
        /// }
        /// ```
        struct Asset3D {
            /// The asset's bytes.
            rerun::components::Blob blob;

            /// The Media Type of the asset.
            ///
            /// Supported values:
            /// * `model/gltf-binary`
            /// * `model/obj` (.mtl material files are not supported yet, references are silently
            /// ignored)
            ///
            /// If omitted, the viewer will try to guess from the data blob.
            /// If it cannot guess, it won't be able to render the asset.
            std::optional<rerun::components::MediaType> media_type;

            /// An out-of-tree transform.
            ///
            /// Applies a transformation to the asset itself without impacting its children.
            std::optional<rerun::components::OutOfTreeTransform3D> transform;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];

          public:
            // Extensions to generated type defined in 'asset3d_ext.cpp'

            static std::optional<rerun::components::MediaType> guess_media_type(
                const std::string& path //
            ) {
                std::filesystem::path file_path(path);
                std::string ext = file_path.extension().string();
                std::transform(ext.begin(), ext.end(), ext.begin(), ::tolower);

                if (ext == ".glb") {
                    return rerun::components::MediaType::glb();
                } else if (ext == ".gltf") {
                    return rerun::components::MediaType::gltf();
                } else if (ext == ".obj") {
                    return rerun::components::MediaType::obj();
                } else {
                    return std::nullopt;
                }
            }

            /// Creates a new [`Asset3D`] from the file contents at `path`.
            ///
            /// The [`MediaType`] will be guessed from the file extension.
            ///
            /// If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess
            /// one from the data at render-time. If it can't, rendering will fail with an error.
            static Asset3D from_file(const std::filesystem::path& path) {
                std::ifstream file(path, std::ios::binary);
                if (!file) {
                    throw std::runtime_error("Failed to open file: " + path.string());
                }

                file.seekg(0, std::ios::end);
                std::streampos length = file.tellg();
                file.seekg(0, std::ios::beg);

                std::vector<uint8_t> data(static_cast<size_t>(length));
                file.read(reinterpret_cast<char*>(data.data()), length);

                return Asset3D::from_bytes(data, Asset3D::guess_media_type(path));
            }

            /// Creates a new [`Asset3D`] from the given `bytes`.
            ///
            /// If no [`MediaType`] is specified, the Rerun Viewer will try to guess one from the
            /// data at render-time. If it can't, rendering will fail with an error.
            static Asset3D from_bytes(
                const std::vector<uint8_t> bytes,
                std::optional<rerun::components::MediaType> media_type
            ) {
                // TODO(cmc): we could try and guess using magic bytes here, like rust does.
                Asset3D asset = Asset3D(bytes);
                asset.media_type = media_type;
                return asset;
            }

          public:
            Asset3D() = default;

            Asset3D(rerun::components::Blob _blob) : blob(std::move(_blob)) {}

            /// The Media Type of the asset.
            ///
            /// Supported values:
            /// * `model/gltf-binary`
            /// * `model/obj` (.mtl material files are not supported yet, references are silently
            /// ignored)
            ///
            /// If omitted, the viewer will try to guess from the data blob.
            /// If it cannot guess, it won't be able to render the asset.
            Asset3D& with_media_type(rerun::components::MediaType _media_type) {
                media_type = std::move(_media_type);
                return *this;
            }

            /// An out-of-tree transform.
            ///
            /// Applies a transformation to the asset itself without impacting its children.
            Asset3D& with_transform(rerun::components::OutOfTreeTransform3D _transform) {
                transform = std::move(_transform);
                return *this;
            }

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates an `AnonymousComponentBatch` out of the associated indicator component. This
            /// allows for associating arbitrary indicator components with arbitrary data. Check out
            /// the `manual_indicator` API example to see what's possible.
            static AnonymousComponentBatch indicator();

            /// Collections all component lists into a list of component collections. *Attention:*
            /// The returned vector references this instance and does not take ownership of any
            /// data. Adding any new components to this archetype will invalidate the returned
            /// component lists!
            std::vector<AnonymousComponentBatch> as_component_batches() const;
        };
    } // namespace archetypes
} // namespace rerun
