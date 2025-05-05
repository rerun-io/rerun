cmake_minimum_required(VERSION 3.15)
project(RerunSdkTest CXX)

find_package(rerun_sdk REQUIRED)

add_executable(example src/example.cpp)
target_link_libraries(example PRIVATE rerun_sdk)
target_compile_features(example PRIVATE cxx_std_17)
