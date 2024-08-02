// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/depth_image.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/blob.hpp"
#include "../components/channel_datatype.hpp"
#include "../components/colormap.hpp"
#include "../components/depth_meter.hpp"
#include "../components/draw_order.hpp"
#include "../components/fill_ratio.hpp"
#include "../components/resolution2d.hpp"
#include "../data_cell.hpp"
#include "../image_utils.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A depth image, i.e. as captured by a depth camera.
    ///
    /// Each pixel corresponds to a depth value in units specified by `components::DepthMeter`.
    ///
    /// Since the underlying `rerun::datatypes::TensorData` uses `rerun::Collection` internally,
    /// data can be passed in without a copy from raw pointers or by reference from `std::vector`/`std::array`/c-arrays.
    /// If needed, this "borrow-behavior" can be extended by defining your own `rerun::CollectionAdapter`.
    ///
    /// ## Example
    ///
    /// ### Depth to 3D example
    /// ![image](https://static.rerun.io/depth_image_3d/924e9d4d6a39d63d4fdece82582855fdaa62d15e/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// #include <algorithm> // fill_n
    /// #include <vector>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_depth_image_3d");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     // Create a synthetic depth image.
    ///     const int HEIGHT = 200;
    ///     const int WIDTH = 300;
    ///     std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
    ///     for (auto y = 50; y <150; ++y) {
    ///         std::fill_n(data.begin() + y * WIDTH + 50, 100, static_cast<uint16_t>(20000));
    ///     }
    ///     for (auto y = 130; y <180; ++y) {
    ///         std::fill_n(data.begin() + y * WIDTH + 100, 180, static_cast<uint16_t>(45000));
    ///     }
    ///
    ///     // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    ///     rec.log(
    ///         "world/camera",
    ///         rerun::Pinhole::from_focal_length_and_resolution(
    ///             200.0f,
    ///             {static_cast<float>(WIDTH), static_cast<float>(HEIGHT)}
    ///         )
    ///     );
    ///
    ///     rec.log(
    ///         "world/camera/depth",
    ///         rerun::DepthImage(data.data(), {WIDTH, HEIGHT})
    ///             .with_meter(10000.0)
    ///             .with_colormap(rerun::components::Colormap::Viridis)
    ///     );
    /// }
    /// ```
    struct DepthImage {
        /// The raw depth image data.
        rerun::components::Blob data;

        /// The size of the image
        rerun::components::Resolution2D resolution;

        /// The data type of the depth image data (U16, F32, …).
        rerun::components::ChannelDatatype datatype;

        /// An optional floating point value that specifies how long a meter is in the native depth units.
        ///
        /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
        /// and a range of up to ~65 meters (2^16 / 1000).
        ///
        /// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
        /// In 3D views on the other hand, this affects where the points of the point cloud are placed.
        std::optional<rerun::components::DepthMeter> meter;

        /// Colormap to use for rendering the depth image.
        ///
        /// If not set, the depth image will be rendered using the Turbo colormap.
        std::optional<rerun::components::Colormap> colormap;

        /// Scale the radii of the points in the point cloud generated from this image.
        ///
        /// A fill ratio of 1.0 (the default) means that each point is as big as to touch the center of its neighbor
        /// if it is at the same depth, leaving no gaps.
        /// A fill ratio of 0.5 means that each point touches the edge of its neighbor if it has the same depth.
        ///
        /// TODO(#6744): This applies only to 3D views!
        std::optional<rerun::components::FillRatio> point_fill_ratio;

        /// An optional floating point value that specifies the 2D drawing order, used only if the depth image is shown as a 2D image.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<rerun::components::DrawOrder> draw_order;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.DepthImageIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public: // START of extensions from depth_image_ext.cpp:
        /// Constructs image from pointer + resolution, inferring the datatype from the pointer type.
        ///
        /// @param pixels The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The length of the data should be `W * H`.
        template <typename TElement>
        DepthImage(const TElement* pixels, components::Resolution2D resolution_)
            : DepthImage{
                  reinterpret_cast<const uint8_t*>(pixels), resolution_, get_datatype(pixels)} {}

        /// Constructs image from pixel data + resolution with datatype inferred from the passed collection.
        ///
        /// @param pixels The raw image data.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        template <typename TElement>
        DepthImage(Collection<TElement> pixels, components::Resolution2D resolution_)
            : DepthImage{pixels.to_uint8(), resolution_, get_datatype(pixels.data())} {}

        /// Constructs image from pixel data + resolution with explicit datatype. Borrows data from a pointer (i.e. data must outlive the image!).
        ///
        /// @param data_ The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The length of the data should be `W * H * datatype.size`
        DepthImage(
            const void* data_, components::Resolution2D resolution_,
            components::ChannelDatatype datatype_
        )
            : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, datatype_))},
              resolution{resolution_},
              datatype{datatype_} {}

        /// Constructs image from pixel data + resolution + datatype.
        ///
        /// @param pixels The raw image data.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        DepthImage(
            Collection<uint8_t> data_, components::Resolution2D resolution_,
            components::ChannelDatatype datatype_
        )
            : data{data_}, resolution{resolution_}, datatype{datatype_} {}

        // END of extensions from depth_image_ext.cpp, start of generated code:

      public:
        DepthImage() = default;
        DepthImage(DepthImage&& other) = default;

        /// An optional floating point value that specifies how long a meter is in the native depth units.
        ///
        /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
        /// and a range of up to ~65 meters (2^16 / 1000).
        ///
        /// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
        /// In 3D views on the other hand, this affects where the points of the point cloud are placed.
        DepthImage with_meter(rerun::components::DepthMeter _meter) && {
            meter = std::move(_meter);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Colormap to use for rendering the depth image.
        ///
        /// If not set, the depth image will be rendered using the Turbo colormap.
        DepthImage with_colormap(rerun::components::Colormap _colormap) && {
            colormap = std::move(_colormap);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Scale the radii of the points in the point cloud generated from this image.
        ///
        /// A fill ratio of 1.0 (the default) means that each point is as big as to touch the center of its neighbor
        /// if it is at the same depth, leaving no gaps.
        /// A fill ratio of 0.5 means that each point touches the edge of its neighbor if it has the same depth.
        ///
        /// TODO(#6744): This applies only to 3D views!
        DepthImage with_point_fill_ratio(rerun::components::FillRatio _point_fill_ratio) && {
            point_fill_ratio = std::move(_point_fill_ratio);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// An optional floating point value that specifies the 2D drawing order, used only if the depth image is shown as a 2D image.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        DepthImage with_draw_order(rerun::components::DrawOrder _draw_order) && {
            draw_order = std::move(_draw_order);
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
    struct AsComponents<archetypes::DepthImage> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::DepthImage& archetype);
    };
} // namespace rerun
