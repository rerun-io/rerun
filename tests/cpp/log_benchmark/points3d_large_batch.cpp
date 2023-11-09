#include <utility>

#include "benchmarks.hpp"
#include "points3d_shared.hpp"
#include "profile_scope.hpp"

#include <rerun.hpp>

constexpr int64_t NUM_POINTS = 50000000;

static void execute(Point3DInput input) {
    PROFILE_FUNCTION();

    rerun::RecordingStream rec("rerun_example_benchmark_points3d_large_batch");

    rec.log(
        "large_batch",
        rerun::Points3D(input.positions)
            .with_colors(input.colors)
            .with_radii(input.radii)
            .with_labels({input.label})
    );
}

void run_points3d_large_batch() {
    PROFILE_FUNCTION();
    auto input = prepare_points3d(42, NUM_POINTS);
    execute(std::move(input));
}
