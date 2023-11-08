#pragma once

#include <cstdint>
#include <string>
#include <vector>
#include "benchmarks.hpp"
#include "profile_scope.hpp"

#include <rerun.hpp>

struct MyPoint3D {
    float x, y, z;
};

struct Point3DInput {
    std::vector<MyPoint3D> positions;
    std::vector<uint32_t> colors;
    std::vector<float> radii;
    std::string label;

    Point3DInput() = default;
    Point3DInput(Point3DInput&&) = default;
};

inline Point3DInput prepare_points3d(int64_t lcg_state, size_t num_points) {
    PROFILE_FUNCTION();

    Point3DInput input;
    input.positions.resize(num_points);
    for (auto& pos : input.positions) {
        pos.x = static_cast<float>(lcg(lcg_state));
        pos.y = static_cast<float>(lcg(lcg_state));
        pos.z = static_cast<float>(lcg(lcg_state));
    }
    input.colors.resize(num_points);
    for (auto& color : input.colors) {
        color = static_cast<uint32_t>(lcg(lcg_state));
    }
    input.radii.resize(num_points);
    for (auto& radius : input.radii) {
        radius = static_cast<float>(lcg(lcg_state));
    }
    input.label = "some label";

    return input;
}

// TODO(andreas): We want this adapter in rerun, ideally in a generated manner.
//                Can we do something like a `binary compatible` attribute on fbs that will generate this as well as ctors?
template <>
struct rerun::ComponentBatchAdapter<rerun::Color, std::vector<uint32_t>> {
    ComponentBatch<Color> operator()(const std::vector<uint32_t>& container) {
        return ComponentBatch<Color>::borrow(container.data(), container.size());
    }

    ComponentBatch<Color> operator()(std::vector<uint32_t>&&) {
        throw std::runtime_error("Not implemented for temporaries");
    }
};

template <>
struct rerun::ComponentBatchAdapter<rerun::Position3D, std::vector<MyPoint3D>> {
    ComponentBatch<rerun::Position3D> operator()(const std::vector<MyPoint3D>& container) {
        return ComponentBatch<rerun::Position3D>::borrow(container.data(), container.size());
    }

    ComponentBatch<rerun::Position3D> operator()(std::vector<MyPoint3D>&&) {
        throw std::runtime_error("Not implemented for temporaries");
    }
};

template <>
struct rerun::ComponentBatchAdapter<rerun::Position3D, MyPoint3D> {
    ComponentBatch<rerun::Position3D> operator()(const MyPoint3D& single) {
        return ComponentBatch<rerun::Position3D>::borrow(&single, 1);
    }

    ComponentBatch<rerun::Position3D> operator()(MyPoint3D&&) {
        throw std::runtime_error("Not implemented for temporaries");
    }
};
