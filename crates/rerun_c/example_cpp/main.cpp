#include <iostream>

#define RERUN_WITH_ARROW 1

#include <rerun.hpp>

int main(int argc, char** argv) {
    loguru::init(argc, argv); // installs signal handlers

    std::cerr << "Rerun C++ SDK version:" << rerun::version_string()
              << std::endl;

    float xyz[9] = {0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 5.0, 5.0, 5.0};
    auto points = rerun::points3(3, xyz).ValueOrDie();
    auto buffer = rerun::ipc_from_table(*points).ValueOrDie();

    std::cerr << "Buffer size: " << buffer->size() << " bytes." << std::endl;
}
