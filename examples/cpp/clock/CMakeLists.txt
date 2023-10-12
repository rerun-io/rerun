cmake_minimum_required(VERSION 3.16)

# project(ClockExample)

# ------------------------------------------------------------------------------

# Arrow requires a C++17 compliant compiler
if(NOT DEFINED CMAKE_CXX_STANDARD)
  set(CMAKE_CXX_STANDARD 17)
endif()

add_executable(clock_example main.cpp)

set_default_warning_settings(clock_example)
target_link_libraries(clock_example PRIVATE rerun_sdk)
