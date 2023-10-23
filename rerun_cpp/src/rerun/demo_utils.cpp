#include "demo_utils.hpp"
#include <algorithm>

namespace rerun {
    namespace demo {

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

        void color_spiral(
            size_t num_points, float radius, float angular_step, float angular_offset, float z_step,
            std::vector<components::Position3D>& out_points,
            std::vector<components::Color>& out_colors
        ) {
            out_points.reserve(num_points);
            out_colors.reserve(num_points);

            for (size_t i = 0; i < num_points; ++i) {
                float angle = static_cast<float>(i) * angular_step * TAU + angular_offset;
                out_points.push_back({
                    sinf(angle) * radius,
                    cosf(angle) * radius,
                    static_cast<float>(i) * z_step,
                });

                out_colors.push_back(colormap_turbo_srgb(static_cast<float>(i) / num_points));
            }
        }

        template <size_t N>
        static float dot(const float (&a)[N], const float (&b)[N]) {
            float sum = 0.0f;
            for (size_t i = 0; i < N; ++i) {
                sum += a[i] * b[i];
            }
            return sum;
        }

        rerun::components::Color colormap_turbo_srgb(float t) {
            const float R[] = {
                0.13572138f,
                4.61539260f,
                -42.66032258f,
                132.13108234f,
                -152.94239396f,
                59.28637943f,
            };
            const float G[] = {
                0.09140261f,
                2.19418839f,
                4.84296658f,
                -14.18503333f,
                4.27729857f,
                2.82956604f,
            };
            const float B[] = {
                0.10667330f,
                12.64194608f,
                -60.58204836f,
                110.36276771f,
                -89.90310912f,
                27.34824973f,
            };

            assert(0.0f <= t && t <= 1.0f);

            const float v[] = {1.0f, t, t * t, t * t * t, t * t * t * t, t * t * t * t * t};

            return rerun::components::Color(
                static_cast<uint8_t>(std::min(std::max(0.0f, dot(v, R)), 1.0f) * 255.0f),
                static_cast<uint8_t>(std::min(std::max(0.0f, dot(v, G)), 1.0f) * 255.0f),
                static_cast<uint8_t>(std::min(std::max(0.0f, dot(v, B)), 1.0f) * 255.0f)
            );
        }
    } // namespace demo
} // namespace rerun
