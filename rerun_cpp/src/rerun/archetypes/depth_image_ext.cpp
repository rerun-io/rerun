#include "../error.hpp"
#include "depth_image.hpp"

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

#include "../image_utils.hpp"

    /// Row-major. Borrows.
    ///
    /// The length of the data should be `W * H`.
    template <typename TElement>
    DepthImage(const TElement* pixels, components::Resolution2D resolution_)
        : DepthImage{reinterpret_cast<const uint8_t*>(pixels), resolution_, get_datatype(pixels)} {}

    /// Row-major.
    ///
    /// The length of the data should be `W * H`.
    template <typename TElement>
    DepthImage(std::vector<TElement> pixels, components::Resolution2D resolution_)
        : DepthImage{Collection<TElement>::take_ownership(std::move(pixels)), resolution_} {}

    /// Row-major.
    ///
    /// The length of the data should be `W * H`.
    template <typename TElement>
    DepthImage(Collection<TElement> pixels, components::Resolution2D resolution_)
        : DepthImage{pixels.to_uint8(), resolution_, get_datatype(pixels.data())} {}

    /// Row-major. Borrows.
    ///
    /// The length of the data should be `W * H * datatype.size`
    DepthImage(
        const void* data_, components::Resolution2D resolution_,
        components::ChannelDatatype datatype_
    )
        : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, datatype_))},
          resolution{resolution_},
          datatype{datatype_} {}

    /// The length of the data should be `W * H * datatype.size`
    DepthImage(
        Collection<uint8_t> data_, components::Resolution2D resolution_,
        components::ChannelDatatype datatype_
    )
        : data{data_}, resolution{resolution_}, datatype{datatype_} {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
