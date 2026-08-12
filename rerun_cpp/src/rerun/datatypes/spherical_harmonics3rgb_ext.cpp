#include "../half.hpp"
#include "spherical_harmonics3rgb.hpp"

namespace rerun::datatypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// The number of coefficients of degrees 1 through 3, i.e. the number of RGB triples.
    static constexpr size_t NUM_COEFFICIENTS = 15;

    /// Construct from 15 RGB coefficient triples of `float`, converting each to half-precision.
    SphericalHarmonics3Rgb(const std::array<std::array<float, 3>, NUM_COEFFICIENTS>& coefficients_);

    // </CODEGEN_COPY_TO_HEADER>
#endif

    SphericalHarmonics3Rgb::SphericalHarmonics3Rgb(
        const std::array<std::array<float, 3>, NUM_COEFFICIENTS>& coefficients_
    ) {
        for (size_t coefficient = 0; coefficient < NUM_COEFFICIENTS; ++coefficient) {
            for (size_t channel = 0; channel < 3; ++channel) {
                coefficients[coefficient][channel] =
                    rerun::half::from_float(coefficients_[coefficient][channel]);
            }
        }
    }

} // namespace rerun::datatypes
