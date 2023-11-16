cmake_minimum_required(VERSION 3.16...3.27)

file(GLOB LOG_BENCHMARK_SOURCES LIST_DIRECTORIES true ${CMAKE_CURRENT_SOURCE_DIR}/*)

add_executable(log_benchmark ${LOG_BENCHMARK_SOURCES})
rerun_strict_warning_settings(log_benchmark)
target_link_libraries(log_benchmark PRIVATE rerun_sdk)
