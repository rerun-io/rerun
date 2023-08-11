cmake_minimum_required(VERSION 3.16)

project(rerun_cpp_proj LANGUAGES CXX)

function(set_default_warning_settings target)
    if(MSVC)
        target_compile_options(${target} PRIVATE /W4 /WX)
    else()
        # Enabled warnings.
        target_compile_options(${target} PRIVATE
            -Wall
            -Wextra
            -Wpedantic
            -Wcast-align
            -Wcast-qual
            -Wformat=2
            -Wmissing-include-dirs
            -Wnull-dereference
            -Wpointer-arith
            -Wshadow
            -Wswitch-enum
            -Wvla
            -Wold-style-cast
        )

        if(CMAKE_CXX_COMPILER_ID MATCHES "Clang") # match both "Clang" and "AppleClang"
            target_compile_options(${target} PRIVATE
                -Wc++17-compat-pedantic
                -Wshadow-all
                -Wnon-gcc
                -Wgnu
                -Wc99-extensions
            )
        endif()

        # Disabled warnings
        # arrow has a bunch of unused parameters in its headers.
        target_compile_options(${target} PRIVATE -Wno-unused-parameter)
    endif()
endfunction()

# Use makefiles on linux, otherwise it might use Ninja which isn't installed by default.
if(NOT DEFINED CMAKE_GENERATOR AND UNIX)
    set(CMAKE_GENERATOR "Unix Makefiles")
endif()

# Arrow requires a C++17 compliant compiler.
if(NOT DEFINED CMAKE_CXX_STANDARD)
    set(CMAKE_CXX_STANDARD 17)
endif()

add_subdirectory(rerun_cpp) # The Rerun C++ SDK library
add_subdirectory(examples/cpp/minimal)
add_subdirectory(tests/cpp)
