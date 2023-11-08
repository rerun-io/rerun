#include <utility>

#include "benchmarks.hpp"
#include "points3d_shared.hpp"
#include "profile_scope.hpp"

#include <rerun.hpp>

constexpr int64_t NUM_POINTS = 1000000;

static void execute(Point3DInput input) {
    PROFILE_FUNCTION();

    rerun::RecordingStream rec("rerun_example_benchmark_points3d_many_individual");

    for (size_t i = 0; i < NUM_POINTS; ++i) {
        rec.set_time_sequence("my_timeline", static_cast<int64_t>(i));
        rec.log(
            "large_batch",
            rerun::Points3D(input.positions[i])
                .with_colors({input.colors[i]})
                .with_radii({input.radii[i]})
        );
    }
}

void run_points3d_many_individual() {
    PROFILE_FUNCTION();
    auto input = prepare_points3d(1337, NUM_POINTS);
    execute(std::move(input));
}
