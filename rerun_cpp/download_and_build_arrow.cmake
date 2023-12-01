# Downloads and builds Apache Arrow from source.
#
# Populates `rerun_arrow_target` with the final arrow target.
# Tries to build an as small as possible version of Arrow that is compatible with the Rerun C++ SDK.
function(download_and_build_arrow)
    include(ExternalProject)

    set(ARROW_DOWNLOAD_PATH ${CMAKE_BINARY_DIR}/arrow)

    if(RERUN_ARROW_LINK_SHARED)
        set(ARROW_BUILD_SHARED ON)
        set(ARROW_BUILD_STATIC OFF)

        if(APPLE)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.dylib)
        elseif(UNIX) # if(LINUX) # CMake 3.25
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.so)
        elseif(WIN32)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/bin/arrow.dll)
        else()
            message(FATAL_ERROR "Unsupported platform.")
        endif()
    else()
        set(ARROW_BUILD_SHARED OFF)
        set(ARROW_BUILD_STATIC ON)

        if(APPLE)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.a)
            set(ARROW_BUNDLED_DEPENDENCIES_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow_bundled_dependencies.a)
        elseif(UNIX) # if(LINUX) # CMake 3.25
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow.a)
            set(ARROW_BUNDLED_DEPENDENCIES_FILE ${ARROW_DOWNLOAD_PATH}/lib/libarrow_bundled_dependencies.a)
        elseif(WIN32)
            set(ARROW_LIBRARY_FILE ${ARROW_DOWNLOAD_PATH}/lib/arrow_static.lib)
            set(ARROW_BUNDLED_DEPENDENCIES_FILE ${ARROW_DOWNLOAD_PATH}/lib/arrow_bundled_dependencies.lib)
        else()
            message(FATAL_ERROR "Unsupported platform.")
        endif()
    endif()

    # Enable multithreaded compiling of Arrow on MSVC.
    if(MSVC)
        # Enable multithreaded compiling of Arrow on MSVC.
        set(ARROW_CXXFLAGS "/MP")

        # ASAN doesn't work with arrow (yet?)
        set(ARROW_ASAN OFF)
    else()
        set(ARROW_CXXFLAGS "")
        set(ARROW_ASAN ${RERUN_USE_ASAN})
    endif()

    # Workaround for https://github.com/apache/arrow/issues/36117
    # This works around linking issues on Windows we got after enabling mimalloc.
    if(MSVC)
        file(MAKE_DIRECTORY ${ARROW_DOWNLOAD_PATH}/src/arrow_cpp-build/debug/)
        file(MAKE_DIRECTORY ${ARROW_DOWNLOAD_PATH}/src/arrow_cpp-build/relwithdebinfo/)
        file(MAKE_DIRECTORY ${ARROW_DOWNLOAD_PATH}/src/arrow_cpp-build/release/)
    endif()

    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        set(ARROW_CMAKE_PRESET ninja-debug-minimal)
    else()
        set(ARROW_CMAKE_PRESET ninja-release-minimal)
    endif()

    ExternalProject_Add(
        arrow_cpp
        PREFIX ${ARROW_DOWNLOAD_PATH}
        GIT_REPOSITORY https://github.com/apache/arrow.git
        GIT_TAG apache-arrow-10.0.1
        GIT_SHALLOW ON
        GIT_PROGRESS OFF # Git progress sounds like a nice idea but is in practive very spammy.

        # LOG_X ON means that the output of the command will
        # be logged to a file _instead_ of printed to the console.
        LOG_CONFIGURE ON
        LOG_BUILD ON
        LOG_INSTALL ON

        CMAKE_ARGS
        --preset ${ARROW_CMAKE_PRESET}
        -DARROW_BOOST_USE_SHARED=OFF
        -DARROW_BUILD_SHARED=${ARROW_BUILD_SHARED}
        -DARROW_BUILD_STATIC=${ARROW_BUILD_STATIC}
        -DARROW_CXXFLAGS=${DARROW_CXXFLAGS}
        -DARROW_IPC=OFF
        -DARROW_JEMALLOC=OFF # We encountered some build issues with jemalloc, use mimalloc instead.
        -DARROW_MIMALLOC=ON
        -DARROW_USE_ASAN=${RERUN_USE_ASAN}
        -DARROW_USE_TSAN=OFF
        -DARROW_USE_UBSAN=OFF
        -DBOOST_SOURCE=BUNDLED
        -DCMAKE_INSTALL_PREFIX=${ARROW_DOWNLOAD_PATH}
        -Dxsimd_SOURCE=BUNDLED
        -DBOOST_SOURCE=BUNDLED
        -DARROW_BOOST_USE_SHARED=OFF
        -DARROW_CXXFLAGS=${DARROW_CXXFLAGS}
        SOURCE_SUBDIR cpp
        BUILD_BYPRODUCTS ${ARROW_LIBRARY_FILE} ${ARROW_BUNDLED_DEPENDENCIES_FILE}
    )

    # arrow_cpp target is not a library. Assemble one from it.
    if(RERUN_ARROW_LINK_SHARED)
        add_library(rerun_arrow_target SHARED IMPORTED GLOBAL)

        # For windows we need to know both the dll AND the import library.
        if(WIN32)
            set_target_properties(rerun_arrow_target PROPERTIES IMPORTED_IMPLIB ${ARROW_DOWNLOAD_PATH}/lib/arrow.lib)
        endif()
    else()
        add_library(rerun_arrow_target STATIC IMPORTED GLOBAL)

        # Need to set the ARROW_STATIC define, otherwise arrow functions are dllimport decorated on Windows.
        target_compile_definitions(rerun_arrow_target INTERFACE ARROW_STATIC)

        # We have to explicitly opt in the arrow bundled dependencies, otherwise we're missing the symbols for mimalloc.
        add_library(arrow_targetBundledDeps STATIC IMPORTED)
        add_dependencies(arrow_targetBundledDeps arrow_cpp)
        set_target_properties(arrow_targetBundledDeps PROPERTIES
            IMPORTED_LOCATION ${ARROW_BUNDLED_DEPENDENCIES_FILE}
        )
        target_link_libraries(rerun_arrow_target INTERFACE arrow_targetBundledDeps)
    endif()

    add_dependencies(rerun_arrow_target arrow_cpp)
    set_target_properties(rerun_arrow_target PROPERTIES
        IMPORTED_LOCATION ${ARROW_LIBRARY_FILE}
        INTERFACE_INCLUDE_DIRECTORIES ${ARROW_DOWNLOAD_PATH}/include
    )

    # Hack to propagate INTERFACE_INCLUDE_DIRECTORIES.
    # via https://stackoverflow.com/a/47358004
    file(MAKE_DIRECTORY ${ARROW_DOWNLOAD_PATH}/include)
endfunction()
