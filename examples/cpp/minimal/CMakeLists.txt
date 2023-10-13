cmake_minimum_required(VERSION 3.16)

add_executable(example_minimal main.cpp)

set_default_warning_settings(example_minimal)
target_link_libraries(example_minimal PRIVATE rerun_sdk)
