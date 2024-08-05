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
    template <typename TElement>
    SegmentationImage(const TElement* pixels, components::Resolution2D resolution_)
        : SegmentationImage{
              reinterpret_cast<const uint8_t*>(pixels), resolution_, get_datatype(pixels)
          } {}

    /// Constructs image from pixel data + resolution with datatype inferred from the passed collection.
    ///
    /// @param pixels The raw image data.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    template <typename TElement>
    SegmentationImage(Collection<TElement> pixels, components::Resolution2D resolution_)
        : SegmentationImage{pixels.to_uint8(), resolution_, get_datatype(pixels.data())} {}

    /// Constructs image from pixel data + resolution with explicit datatype. Borrows data from a pointer (i.e. data must outlive the image!).
    ///
    /// @param data_ The raw image data.
    /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
    /// The byte size of the data is assumed to be `W * H * datatype.size`
    SegmentationImage(
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
    SegmentationImage(
        Collection<uint8_t> data_, components::Resolution2D resolution_,
        components::ChannelDatatype datatype_
    )
        : data{data_}, resolution{resolution_}, datatype{datatype_} {}

    // </CODEGEN_COPY_TO_HEADER>

} // namespace rerun::archetypes

#endif
