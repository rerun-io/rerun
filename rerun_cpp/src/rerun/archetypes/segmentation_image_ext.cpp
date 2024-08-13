#if 0

#include "segmentation_image.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../image_utils.hpp"

// </CODEGEN_COPY_TO_HEADER>
namespace rerun::archetypes {
    // <CODEGEN_COPY_TO_HEADER>

    /// Constructs image from pointer + resolution, inferring the datatype from the pointer type.
    ///
    /// @param pixels The raw image data.
    /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
    /// The number of elements is assumed to be `W * H`.
    /// @param resolution The resolution of the image as {width, height}.
    template <typename TElement>
    SegmentationImage(const TElement* pixels, WidthHeight resolution)
        : SegmentationImage{
              reinterpret_cast<const uint8_t*>(pixels), resolution, get_datatype(pixels)
          } {}

    /// Constructs image from pixel data + resolution with datatype inferred from the passed collection.
    ///
    /// @param pixels The raw image data.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    /// @param resolution The resolution of the image as {width, height}.
    template <typename TElement>
    SegmentationImage(Collection<TElement> pixels, WidthHeight resolution)
        : SegmentationImage{pixels.to_uint8(), resolution, get_datatype(pixels.data())} {}

    /// Constructs image from pixel data + resolution with explicit datatype. Borrows data from a pointer (i.e. data must outlive the image!).
    ///
    /// @param bytes The raw image data.
    /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
    /// The byte size of the data is assumed to be `W * H * datatype.size`
    /// @param resolution The resolution of the image as {width, height}.
    /// @param datatype How the data should be interpreted.
    SegmentationImage(
        const void* bytes, WidthHeight resolution,
        datatypes::ChannelDatatype datatype
    )
        : SegmentationImage{Collection<uint8_t>::borrow(bytes, num_bytes(resolution, datatype)), resolution, datatype} {}

    /// Constructs image from pixel data + resolution + datatype.
    ///
    /// @param bytes The raw image data as bytes.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    /// @param resolution The resolution of the image as {width, height}.
    /// @param datatype How the data should be interpreted.
    SegmentationImage(
        Collection<uint8_t> bytes, WidthHeight resolution,
        datatypes::ChannelDatatype datatype
    )
        : buffer{bytes}, format{datatypes::ImageFormat{resolution, datatype}} {
            if (buffer.size() != format.image_format.num_bytes()) {
                Error(
                    ErrorCode::InvalidTensorDimension,
                    "SegmentationImage buffer has the wrong size. Got " + std::to_string(buffer.size()) +
                        " bytes, expected " + std::to_string(format.image_format.num_bytes())
                )
                    .handle();
            }
        }

    // </CODEGEN_COPY_TO_HEADER>

} // namespace rerun::archetypes

#endif
