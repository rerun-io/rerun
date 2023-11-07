// Simple benchmark suite for logging data.
// The goal is to get an estimate for the entire process of logging data,
// including serialization and processing by the recording stream.
//
// Timings are printed out while running, it's recommended to measure process run time to ensure
// we account for all startup overheads and have all background threads finish.
//
// If not specified otherwise, memory recordings are used.
//
// The data we generate for benchmarking should be:
// * minimal overhead to generate
// * not homogenous (arrow, ourselves, or even the compiler might exploit this)
// * not trivially optimized out
// * not random between runs
//
// Run all benchmarks using:
// ```
// pixi run cpp-log-benchmark
// ```
// Or, run a single benchmark using:
// ```
// pixi run cpp-log-benchmark points3d_large_batch
// ```
//

#include <cstdio>
#include <vector>

#include "benchmarks.hpp"

int64_t lcg(int64_t& lcg_state) {
    lcg_state = 1140671485 * lcg_state + 128201163 % 16777216;
    return lcg_state;
}

int main(int argc, char** argv) {
    std::vector<const char*> benchmarks(argv + 1, argv + argc);
    if (argc == 1) {
        benchmarks.push_back(ArgPoints3DLargeBatch);
        benchmarks.push_back(ArgPoints3DManyIndividual);
        benchmarks.push_back(ArgImage);
    }

    for (const auto& benchmark : benchmarks) {
        if (strcmp(benchmark, ArgPoints3DLargeBatch) == 0) {
            run_points3d_large_batch();
        } else if (benchmark == ArgPoints3DManyIndividual) {
            run_points3d_many_individual();
        } else if (benchmark == ArgImage) {
            run_image();
        } else {
            printf("Unknown benchmark: %s\n", benchmark);
            return 1;
        }
    }

    return 0;
}
