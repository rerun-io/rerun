// Utilities used in examples.

#include <algorithm>
#include <cmath>
#include "components/position3d.hpp"

namespace rerun {
    namespace demo {

        /// Linearly interpolates from `a` through `b` in `n` steps, returning the intermediate result at
        /// each step.
        template <typename T>
        std::vector<T> linspace(T start, T end, size_t num) {
            std::vector<T> linspaced(num);
            std::generate(linspaced.begin(), linspaced.end(), [&, i = 0]() mutable {
                return start + i++ * (end - start) / static_cast<T>(num - 1);
            });
            return linspaced;
        }

        /// Given two 3D vectors `from` and `to`, linearly interpolates between them in `n` steps along
        /// the three axes, returning the intermediate result at each step.
        std::vector<components::Position3D> grid(
            components::Position3D from, components::Position3D to, size_t n
        ) {
            std::vector<components::Position3D> output;
            for (float z : linspace(from.z(), to.z(), n)) {
                for (float y : linspace(from.y(), to.y(), n)) {
                    for (float x : linspace(from.x(), to.x(), n)) {
                        output.push_back({x, y, z});
                    }
                }
            }
            return output;
        }

    } // namespace demo
} // namespace rerun
