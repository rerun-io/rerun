cmake_minimum_required(VERSION 3.16)
add_executable(example_clock main.cpp)

set_default_warning_settings(example_clock)
target_link_libraries(example_clock PRIVATE rerun_sdk)
