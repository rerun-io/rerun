cmake_minimum_required(VERSION 3.16...3.27)

file(GLOB PLOT_DASHBOARD_STRESS_SOURCES LIST_DIRECTORIES true ${CMAKE_CURRENT_SOURCE_DIR}/*)

add_executable(plot_dashboard_stress ${PLOT_DASHBOARD_STRESS_SOURCES})
rerun_strict_warning_settings(plot_dashboard_stress)
target_link_libraries(plot_dashboard_stress PRIVATE rerun_sdk)
