function(round_trip_test test_name)
    if(NOT DEFINED CMAKE_CXX_STANDARD)
        set(CMAKE_CXX_STANDARD 17)
    endif()

    add_executable(${test_name} "${test_name}/main.cpp")

    if(MSVC)
        target_compile_options(${test_name} PRIVATE /W4 /WX)
    else()
        target_compile_options(${test_name} PRIVATE -Wall -Wextra -Wpedantic -Wcast-align -Wcast-qual -Wformat=2 -Wmissing-include-dirs -Wnull-dereference -Woverloaded-virtual -Wpointer-arith -Wshadow -Wswitch-enum -Wvla -Wno-sign-compare -Wconversion -Wunused -Wold-style-cast -Wno-missing-braces)
    endif()

    include_directories(SYSTEM ${CMAKE_CURRENT_SOURCE_DIR}/../../../rerun_cpp/src) # For rerun.hpp (Rerun C++ SDK)

    target_link_libraries(${test_name} PRIVATE rerun_sdk)
endfunction()

# Needs Rect2D for the time being which isn't there yet.
# round_trip_test(points2d)
round_trip_test(points3d)
