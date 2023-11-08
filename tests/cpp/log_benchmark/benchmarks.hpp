#pragma once

#include <cstdint>

static const char* ArgPoints3DLargeBatch = "points3d_large_batch";
static const char* ArgPoints3DManyIndividual = "points3d_many_individual";
static const char* ArgImage = "image";

/// Log a single large batch of points with positions, colors, radii and a splatted string.
void run_points3d_large_batch();

/// Log many individual points (position, color, radius), each with a different timestamp.
void run_points3d_many_individual();

/// Log a few large images.
void run_image();

// ---

/// Very simple linear congruency "random" number generator to spread out values a bit.
inline int64_t lcg(int64_t& lcg_state) {
    lcg_state = 1140671485 * lcg_state + 128201163 % 16777216;
    return lcg_state;
}
