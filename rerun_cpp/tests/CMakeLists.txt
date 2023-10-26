cmake_minimum_required(VERSION 3.16)

# Catch2:
Include(FetchContent)
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.4.0
)
FetchContent_MakeAvailable(Catch2)

file(GLOB_RECURSE rerun_sdk_tests_SRC CONFIGURE_DEPENDS
    "*.hpp"
    "*.cpp"
)
add_executable(rerun_sdk_tests ${rerun_sdk_tests_SRC})

set_default_warning_settings(rerun_sdk_tests)


# -----------------------------------------------------------------------------
# Arrow:
option(ARROW_LINK_SHARED "Link to the Arrow shared library" ON)

# find_package(Arrow REQUIRED)

# Arrow requires a C++17 compliant compiler
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(RERUN_ARROW_LINK_SHARED)
  add_library(ArrowCpp SHARED IMPORTED)
  if(APPLE)
      set_target_properties(ArrowCpp PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/externals/lib/libarrow.so)
  elseif(UNIX) # if(LINUX) # CMake 3.25
      set_target_properties(ArrowCpp PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/externals/lib/libarrow.so)
  elseif(WIN32)
      set_target_properties(ArrowCpp PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/externals/lib/arrow.dll)
  else()
      message(FATAL_ERROR "Unsupported platform.")
  endif()

  target_link_libraries(rerun_sdk_tests PRIVATE ArrowCpp)
else()
  add_library(ArrowCpp STATIC IMPORTED)
    if(APPLE)
        set_target_properties(ArrowCpp PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/externals/lib/libarrow.a)
    elseif(UNIX) # if(LINUX) # CMake 3.25
        set_target_properties(ArrowCpp PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/externals/lib/libarrow.a)
    elseif(WIN32)
        set_target_properties(ArrowCpp PROPERTIES IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/externals/lib/arrow.dll)
    else()
        message(FATAL_ERROR "Unsupported platform.")
    endif()

  target_link_libraries(rerun_sdk_tests PRIVATE ArrowCpp)
endif()

# message(STATUS "Arrow version: ${ARROW_VERSION}")
# message(STATUS "Arrow SO version: ${ARROW_FULL_SO_VERSION}")
#
# if(ARROW_LINK_SHARED)
#     target_link_libraries(rerun_sdk_tests PRIVATE Arrow::arrow_shared)
# else()
#     target_link_libraries(rerun_sdk_tests PRIVATE Arrow::arrow_static)
# endif()

# Include arrow explicitly again, otherwise the arrow headers won't be declared as system headers.
# and instead become regular headers, causing warnings to be emitted.
target_link_libraries(rerun_sdk_tests PRIVATE loguru::loguru Catch2::Catch2 rerun_sdk)
