// Log random points and the corresponding covariance ellipsoid.

#include <rerun.hpp>

#include <algorithm>
#include <random>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_ellipsoid_simple");
    rec.spawn().exit_on_failure();

    const float sigmas[3] = {5.0f, 3.0f, 1.0f};

    std::default_random_engine gen;
    std::normal_distribution<float> dist(0.0, 1.0f);

    std::vector<rerun::Position3D> points3d(50000);
    std::generate(points3d.begin(), points3d.end(), [&] {
        return rerun::Position3D(
            sigmas[0] * dist(gen),
            sigmas[1] * dist(gen),
            sigmas[2] * dist(gen)
        );
    });

    rec.log(
        "points",
        rerun::Points3D(points3d).with_radii(0.02f).with_colors(rerun::Rgba32(188, 77, 185))
    );

    rec.log(
        "ellipsoid",
        rerun::Ellipsoids3D::from_centers_and_half_sizes(
            {
                {0.0f, 0.0f, 0.0f},
                {0.0f, 0.0f, 0.0f},
            },
            {
                {sigmas[0], sigmas[1], sigmas[2]},
                {3.0f * sigmas[0], 3.0f * sigmas[1], 3.0f * sigmas[2]},
            }
        )
            .with_colors({
                rerun::Rgba32(255, 255, 0),
                rerun::Rgba32(64, 64, 0),
            })
    );
}
