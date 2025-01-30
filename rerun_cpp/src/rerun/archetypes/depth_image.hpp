// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/depth_image.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/colormap.hpp"
#include "../components/depth_meter.hpp"
#include "../components/draw_order.hpp"
#include "../components/fill_ratio.hpp"
#include "../components/image_buffer.hpp"
#include "../components/image_format.hpp"
#include "../components/value_range.hpp"
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
    /// Since the underlying `rerun::datatypes::ImageBuffer` uses `rerun::Collection` internally,
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
        std::optional<ComponentBatch> buffer;

        /// The format of the image.
        std::optional<ComponentBatch> format;

        /// An optional floating point value that specifies how long a meter is in the native depth units.
        ///
        /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
        /// and a range of up to ~65 meters (2^16 / 1000).
        ///
        /// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
        /// In 3D views on the other hand, this affects where the points of the point cloud are placed.
        std::optional<ComponentBatch> meter;

        /// Colormap to use for rendering the depth image.
        ///
        /// If not set, the depth image will be rendered using the Turbo colormap.
        std::optional<ComponentBatch> colormap;

        /// The expected range of depth values.
        ///
        /// This is typically the expected range of valid values.
        /// Everything outside of the range is clamped to the range for the purpose of colormpaping.
        /// Note that point clouds generated from this image will still display all points, regardless of this range.
        ///
        /// If not specified, the range will be automatically estimated from the data.
        /// Note that the Viewer may try to guess a wider range than the minimum/maximum of values
        /// in the contents of the depth image.
        /// E.g. if all values are positive, some bigger than 1.0 and all smaller than 255.0,
        /// the Viewer will guess that the data likely came from an 8bit image, thus assuming a range of 0-255.
        std::optional<ComponentBatch> depth_range;

        /// Scale the radii of the points in the point cloud generated from this image.
        ///
        /// A fill ratio of 1.0 (the default) means that each point is as big as to touch the center of its neighbor
        /// if it is at the same depth, leaving no gaps.
        /// A fill ratio of 0.5 means that each point touches the edge of its neighbor if it has the same depth.
        ///
        /// TODO(#6744): This applies only to 3D views!
        std::optional<ComponentBatch> point_fill_ratio;

        /// An optional floating point value that specifies the 2D drawing order, used only if the depth image is shown as a 2D image.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        std::optional<ComponentBatch> draw_order;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.DepthImageIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.DepthImage";

        /// `ComponentDescriptor` for the `buffer` field.
        static constexpr auto Descriptor_buffer = ComponentDescriptor(
            ArchetypeName, "buffer",
            Loggable<rerun::components::ImageBuffer>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `format` field.
        static constexpr auto Descriptor_format = ComponentDescriptor(
            ArchetypeName, "format",
            Loggable<rerun::components::ImageFormat>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `meter` field.
        static constexpr auto Descriptor_meter = ComponentDescriptor(
            ArchetypeName, "meter",
            Loggable<rerun::components::DepthMeter>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `colormap` field.
        static constexpr auto Descriptor_colormap = ComponentDescriptor(
            ArchetypeName, "colormap",
            Loggable<rerun::components::Colormap>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `depth_range` field.
        static constexpr auto Descriptor_depth_range = ComponentDescriptor(
            ArchetypeName, "depth_range",
            Loggable<rerun::components::ValueRange>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `point_fill_ratio` field.
        static constexpr auto Descriptor_point_fill_ratio = ComponentDescriptor(
            ArchetypeName, "point_fill_ratio",
            Loggable<rerun::components::FillRatio>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `draw_order` field.
        static constexpr auto Descriptor_draw_order = ComponentDescriptor(
            ArchetypeName, "draw_order",
            Loggable<rerun::components::DrawOrder>::Descriptor.component_name
        );

      public: // START of extensions from depth_image_ext.cpp:
        /// Constructs image from pointer + resolution, inferring the datatype from the pointer type.
        ///
        /// @param pixels The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The number of elements is assumed to be `W * H`.
        /// @param resolution The resolution of the image as {width, height}.
        template <typename TElement>
        DepthImage(const TElement* pixels, WidthHeight resolution)
            : DepthImage{
                  reinterpret_cast<const uint8_t*>(pixels), resolution, get_datatype(pixels)} {}

        /// Constructs image from pixel data + resolution with datatype inferred from the passed collection.
        ///
        /// @param pixels The raw image data.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        /// @param resolution The resolution of the image as {width, height}.
        template <typename TElement>
        DepthImage(Collection<TElement> pixels, WidthHeight resolution)
            : DepthImage{pixels.to_uint8(), resolution, get_datatype(pixels.data())} {}

        /// Constructs image from pixel data + resolution with explicit datatype. Borrows data from a pointer (i.e. data must outlive the image!).
        ///
        /// @param bytes The raw image data.
        /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
        /// The byte size of the data is assumed to be `W * H * datatype.size`
        /// @param resolution The resolution of the image as {width, height}.
        /// @param datatype How the data should be interpreted.
        DepthImage(const void* bytes, WidthHeight resolution, datatypes::ChannelDatatype datatype)
            : DepthImage{
                  Collection<uint8_t>::borrow(bytes, num_bytes(resolution, datatype)),
                  resolution,
                  datatype} {}

        /// Constructs image from pixel data + resolution + datatype.
        ///
        /// @param bytes The raw image data as bytes.
        /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
        /// explicitly ahead of time with `rerun::Collection::take_ownership`.
        /// The length of the data should be `W * H`.
        /// @param resolution The resolution of the image as {width, height}.
        /// @param datatype How the data should be interpreted.
        DepthImage(
            Collection<uint8_t> bytes, WidthHeight resolution, datatypes::ChannelDatatype datatype
        ) {
            auto image_format = datatypes::ImageFormat{resolution, datatype};
            if (bytes.size() != image_format.num_bytes()) {
                Error(
                    ErrorCode::InvalidTensorDimension,
                    "DepthImage buffer has the wrong size. Got " + std::to_string(bytes.size()) +
                        " bytes, expected " + std::to_string(image_format.num_bytes())
                )
                    .handle();
            }
            *this = std::move(*this).with_buffer(bytes).with_format(image_format);
        }

        // END of extensions from depth_image_ext.cpp, start of generated code:

      public:
        DepthImage() = default;
        DepthImage(DepthImage&& other) = default;
        DepthImage(const DepthImage& other) = default;
        DepthImage& operator=(const DepthImage& other) = default;
        DepthImage& operator=(DepthImage&& other) = default;

        /// Update only some specific fields of a `DepthImage`.
        static DepthImage update_fields() {
            return DepthImage();
        }

        /// Clear all the fields of a `DepthImage`.
        static DepthImage clear_fields();

        /// The raw depth image data.
        DepthImage with_buffer(const rerun::components::ImageBuffer& _buffer) && {
            buffer = ComponentBatch::from_loggable(_buffer, Descriptor_buffer).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `buffer` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_buffer` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_buffer(const Collection<rerun::components::ImageBuffer>& _buffer) && {
            buffer = ComponentBatch::from_loggable(_buffer, Descriptor_buffer).value_or_throw();
            return std::move(*this);
        }

        /// The format of the image.
        DepthImage with_format(const rerun::components::ImageFormat& _format) && {
            format = ComponentBatch::from_loggable(_format, Descriptor_format).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `format` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_format` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_format(const Collection<rerun::components::ImageFormat>& _format) && {
            format = ComponentBatch::from_loggable(_format, Descriptor_format).value_or_throw();
            return std::move(*this);
        }

        /// An optional floating point value that specifies how long a meter is in the native depth units.
        ///
        /// For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
        /// and a range of up to ~65 meters (2^16 / 1000).
        ///
        /// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
        /// In 3D views on the other hand, this affects where the points of the point cloud are placed.
        DepthImage with_meter(const rerun::components::DepthMeter& _meter) && {
            meter = ComponentBatch::from_loggable(_meter, Descriptor_meter).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `meter` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_meter` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_meter(const Collection<rerun::components::DepthMeter>& _meter) && {
            meter = ComponentBatch::from_loggable(_meter, Descriptor_meter).value_or_throw();
            return std::move(*this);
        }

        /// Colormap to use for rendering the depth image.
        ///
        /// If not set, the depth image will be rendered using the Turbo colormap.
        DepthImage with_colormap(const rerun::components::Colormap& _colormap) && {
            colormap =
                ComponentBatch::from_loggable(_colormap, Descriptor_colormap).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `colormap` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_colormap` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_colormap(const Collection<rerun::components::Colormap>& _colormap) && {
            colormap =
                ComponentBatch::from_loggable(_colormap, Descriptor_colormap).value_or_throw();
            return std::move(*this);
        }

        /// The expected range of depth values.
        ///
        /// This is typically the expected range of valid values.
        /// Everything outside of the range is clamped to the range for the purpose of colormpaping.
        /// Note that point clouds generated from this image will still display all points, regardless of this range.
        ///
        /// If not specified, the range will be automatically estimated from the data.
        /// Note that the Viewer may try to guess a wider range than the minimum/maximum of values
        /// in the contents of the depth image.
        /// E.g. if all values are positive, some bigger than 1.0 and all smaller than 255.0,
        /// the Viewer will guess that the data likely came from an 8bit image, thus assuming a range of 0-255.
        DepthImage with_depth_range(const rerun::components::ValueRange& _depth_range) && {
            depth_range = ComponentBatch::from_loggable(_depth_range, Descriptor_depth_range)
                              .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `depth_range` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_depth_range` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_depth_range(
            const Collection<rerun::components::ValueRange>& _depth_range
        ) && {
            depth_range = ComponentBatch::from_loggable(_depth_range, Descriptor_depth_range)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Scale the radii of the points in the point cloud generated from this image.
        ///
        /// A fill ratio of 1.0 (the default) means that each point is as big as to touch the center of its neighbor
        /// if it is at the same depth, leaving no gaps.
        /// A fill ratio of 0.5 means that each point touches the edge of its neighbor if it has the same depth.
        ///
        /// TODO(#6744): This applies only to 3D views!
        DepthImage with_point_fill_ratio(const rerun::components::FillRatio& _point_fill_ratio) && {
            point_fill_ratio =
                ComponentBatch::from_loggable(_point_fill_ratio, Descriptor_point_fill_ratio)
                    .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `point_fill_ratio` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_point_fill_ratio` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_point_fill_ratio(
            const Collection<rerun::components::FillRatio>& _point_fill_ratio
        ) && {
            point_fill_ratio =
                ComponentBatch::from_loggable(_point_fill_ratio, Descriptor_point_fill_ratio)
                    .value_or_throw();
            return std::move(*this);
        }

        /// An optional floating point value that specifies the 2D drawing order, used only if the depth image is shown as a 2D image.
        ///
        /// Objects with higher values are drawn on top of those with lower values.
        DepthImage with_draw_order(const rerun::components::DrawOrder& _draw_order) && {
            draw_order =
                ComponentBatch::from_loggable(_draw_order, Descriptor_draw_order).value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `draw_order` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_draw_order` should
        /// be used when logging a single row's worth of data.
        DepthImage with_many_draw_order(const Collection<rerun::components::DrawOrder>& _draw_order
        ) && {
            draw_order =
                ComponentBatch::from_loggable(_draw_order, Descriptor_draw_order).value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentColumn::from_batch_with_lengths`.
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
    struct AsComponents<archetypes::DepthImage> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::DepthImage& archetype
        );
    };
} // namespace rerun
