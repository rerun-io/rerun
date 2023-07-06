cmake_minimum_required(VERSION 3.16)

# project(RerunExample)

# ------------------------------------------------------------------------------

# Arrow requires a C++17 compliant compiler
if(NOT DEFINED CMAKE_CXX_STANDARD)
  set(CMAKE_CXX_STANDARD 17)
endif()

if(NOT DEFINED CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

add_executable(rerun_example main.cpp)

if(MSVC)
  target_compile_options(rerun_example PRIVATE /W4 /WX)
else()
  target_compile_options(rerun_example PRIVATE -Wall -Wextra -Wpedantic -Wcast-align -Wcast-qual -Wformat=2 -Wmissing-include-dirs -Wnull-dereference -Woverloaded-virtual -Wpointer-arith -Wshadow -Wswitch-enum -Wvla -Wno-sign-compare -Wconversion -Wunused -Wold-style-cast -Wno-missing-braces)
endif()

include_directories(SYSTEM ${CMAKE_CURRENT_SOURCE_DIR}/../src) # For rerun.hpp (Rerun C++ SDK)

target_link_libraries(rerun_example PRIVATE loguru::loguru rerun_sdk)
