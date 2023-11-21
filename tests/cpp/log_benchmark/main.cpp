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
// * not homogeneous (arrow, ourselves, or even the compiler might exploit this)
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
// For better whole-executable timing capture you can also first build the executable and then run:
// ```
// pixi run cpp-build-log-benchmark
// ./build/release/tests/cpp/log_benchmark/log_benchmark
// ```
//

#include <cstdio>
#include <cstring>
#include <vector>

#include "benchmarks.hpp"

static const char* ArgPoints3DLargeBatch = "points3d_large_batch";
static const char* ArgPoints3DManyIndividual = "points3d_many_individual";
static const char* ArgImage = "image";

int main(int argc, char** argv) {
#ifndef NDEBUG
    printf("WARNING: Debug build, timings will be inaccurate!\n");
#endif

    std::vector<const char*> benchmarks(argv + 1, argv + argc);
    if (argc == 1) {
        benchmarks.push_back(ArgPoints3DLargeBatch);
        benchmarks.push_back(ArgPoints3DManyIndividual);
        benchmarks.push_back(ArgImage);
    }

    for (const auto& benchmark : benchmarks) {
        if (strcmp(benchmark, ArgPoints3DLargeBatch) == 0) {
            run_points3d_large_batch();
        } else if (strcmp(benchmark, ArgPoints3DManyIndividual) == 0) {
            run_points3d_many_individual();
        } else if (strcmp(benchmark, ArgImage) == 0) {
            run_image();
        } else {
            printf("Unknown benchmark: %s\n", benchmark);
            return 1;
        }
    }

    return 0;
}
