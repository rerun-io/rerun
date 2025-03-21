#if 0

#include "series_points.hpp"

namespace rerun::archetypes {
    // <CODEGEN_COPY_TO_HEADER>

    // Overload needed to avoid confusion with passing single strings.
    /// Display name of the series.
    ///
    /// Used in the legend. Expected to be unchanging over time.
    SeriesPoints with_names(const char* _name) && {
        names = ComponentBatch::from_loggable(rerun::components::Name(_name), Descriptor_names).value_or_throw();
        return std::move(*this);
    }

    // </CODEGEN_COPY_TO_HEADER>

} // namespace rerun::archetypes

#endif
