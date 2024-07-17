#include "../error.hpp"
#include "segmentation_image.hpp"

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

#include "../image_utils.hpp"

    /// Row-major. Borrows.
    ///
    /// The length of the data should be `W * H`.
    template <typename TElement>
    SegmentationImage(const TElement* pixels, components::Resolution2D resolution_)
        : SegmentationImage{
              reinterpret_cast<const uint8_t*>(pixels), resolution_, get_data_type(pixels)
          } {}

    /// Row-major.
    ///
    /// The length of the data should be `W * H`.
    template <typename TElement>
    SegmentationImage(std::vector<TElement> pixels, components::Resolution2D resolution_)
        : SegmentationImage{Collection<TElement>::take_ownership(std::move(pixels)), resolution_} {}

    /// Row-major.
    ///
    /// The length of the data should be `W * H`.
    template <typename TElement>
    SegmentationImage(Collection<TElement> pixels, components::Resolution2D resolution_)
        : SegmentationImage{pixels.to_uint8(), resolution_, get_data_type(pixels.data())} {}

    /// Row-major. Borrows.
    ///
    /// The length of the data should be `W * H * data_type.size`
    SegmentationImage(
        const void* data_, components::Resolution2D resolution_,
        components::ChannelDataType data_type_
    )
        : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, data_type_))},
          resolution{resolution_},
          data_type{data_type_} {}

    /// New depth image from an `ChannelDataType` and a pointer.
    ///
    /// The length of the data should be `W * H * data_type.size`
    SegmentationImage(
        Collection<uint8_t> data_, components::Resolution2D resolution_,
        components::ChannelDataType data_type_
    )
        : data{data_}, resolution{resolution_}, data_type{data_type_} {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
