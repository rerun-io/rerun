import os
import re
from pathlib import Path

from conan import ConanFile
from conan.tools.cmake import CMake, CMakeDeps, CMakeToolchain, cmake_layout
from conan.tools.files import copy, get, load, rmdir, save
from conan.tools.scm import Git


class RerunCppSdkConan(ConanFile):
    name = "rerun_cpp_sdk"
    # Version will be extracted from sdk_info.h
    version = None
    license = "Apache-2.0"  # Assuming Apache-2.0 based on repository license
    author = "Rerun AI <oss@rerun.io>"
    url = "https://github.com/rerun-io/rerun"
    description = "Rerun C++ SDK for logging computer vision and robotics data."
    topics = ("computer-vision", "robotics", "logging", "visualization")
    settings = "os", "compiler", "build_type", "arch"
    options = {
        "shared": [True, False],
        "fPIC": [True, False],
    }
    default_options = {
        "shared": False,
        "fPIC": True,
    }
    # Only export sources within the rerun_cpp directory
    exports_sources = "CMakeLists.txt", "src/*", "Config.cmake.in"

    def _get_rerun_version(self):
        # Extract version from the C header file relative to the conanfile's location
        sdk_info_h_path = os.path.join(self.recipe_folder, "src", "rerun", "c", "sdk_info.h")
        if os.path.exists(sdk_info_h_path):
            sdk_info_h = load(self, sdk_info_h_path)
            # Capture any version string between quotes
            match = re.search(r'#define\s+RERUN_SDK_HEADER_VERSION\s+"([^"]+)"', sdk_info_h)
            if match:
                return match.group(1)
        self.output.warning("Could not determine version from src/rerun/c/sdk_info.h")
        return "0.0.0-local" # Fallback version

    def set_version(self):
        self.version = self._get_rerun_version()

    def config_options(self):
        if self.settings.os == "Windows":
            del self.options.fPIC

    def layout(self):
        # Use standard cmake layout, build folder inside repository structure
        cmake_layout(self, src_folder=".", build_folder="build/" + str(self.settings.build_type))
        # Adjust layout for source files if needed, assuming conanfile is in rerun_cpp
        self.folders.source = "."
        self.folders.generators = os.path.join(self.folders.build, "conan")

        # Location for rerun_c artifacts relative to build folder
        self.cpp.build.libdirs = ["lib"]


    def requirements(self):
        # Add the rerun_c dependency
        self.requires(f"rerun_c/{self.version}")

        # Explicitly require dependencies to override potential non-standard versions (like .Z, cci)
        # coming from profiles/global.conf that might interfere with `--build=missing`.
        # Versions extracted from the previous error log.
        self.requires("arrow/15.0.0")
        self.requires("boost/1.85.0")
        self.requires("bzip2/1.0.8")
        self.requires("libbacktrace/cci.20210118")
        self.requires("libevent/2.1.12")
        self.requires("openssl/3.4.1")
        self.requires("thrift/0.20.0")
        self.requires("zlib/1.3.1") # Use 1.3.1 as seen resolving in the log

    def validate(self):
        if self.settings.compiler.get_safe("cppstd"):
            # Check C++17 compatibility, adjust compiler versions as needed
            pass # Add checks if necessary

    def build_requirements(self):
        # CMake needed to build
        self.tool_requires("cmake/[>=3.16]")

    def generate(self):
        tc = CMakeToolchain(self)
        # Disable Arrow download, use Conan's Arrow
        tc.variables["RERUN_DOWNLOAD_AND_BUILD_ARROW"] = False

        # Get path to rerun_c from dependencies
        rerun_c_dep_path = self.dependencies["rerun_c"].package_folder
        rerun_c_lib_name = self._get_rerun_c_lib_name()
        rerun_c_lib_path = os.path.join(rerun_c_dep_path, "lib", rerun_c_lib_name)

        # Pass path to rerun_c library to CMake
        tc.variables["RERUN_C_LIB"] = rerun_c_lib_path
        tc.variables["RERUN_INSTALL_RERUN_C"] = True  # Must be true for static library

        tc.generate()

        deps = CMakeDeps(self)
        deps.generate()

    def _get_rerun_c_lib_name(self):
        # Determine the expected library name based on platform
        if self.settings.os == "Macos":
            arch_suffix = "x64" if self.settings.arch == "x86_64" else "arm64"
            return f"librerun_c__macos_{arch_suffix}.a"
        elif self.settings.os == "Linux":
            arch_suffix = "x64" if self.settings.arch == "x86_64" else "arm64"
            return f"librerun_c__linux_{arch_suffix}.a"
        elif self.settings.os == "Windows":
            arch_suffix = "x64" # Assuming only x64 for now
            return f"rerun_c__win_{arch_suffix}.lib"
        return None

    def build(self):
        # Build rerun_sdk using CMake
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        cmake = CMake(self)
        cmake.install() # Use the install logic from CMakeLists

        # Remove CMake config files from lib/cmake, Conan generates its own
        rmdir(self, os.path.join(self.package_folder, "lib", "cmake"))

        # Copy license file from the original source location (relative to recipe folder)
        # Assuming license is in the root of the rerun git repo
        license_src_path = os.path.join(self.recipe_folder, "..", "LICENSE")
        if os.path.exists(license_src_path):
            copy(self, "LICENSE", src=os.path.dirname(license_src_path), dst="licenses")
        else:
            self.output.warning(f"LICENSE file not found at expected location: {license_src_path}")

    def package_info(self):
        self.cpp_info.set_property("cmake_file_name", "rerun_sdk")
        self.cpp_info.set_property("cmake_target_name", "rerun::rerun_sdk")

        # Library name is 'rerun_sdk' as defined in CMakeLists
        self.cpp_info.libs = ["rerun_sdk"]

        # Provide the include directory
        self.cpp_info.includedirs = ["include"]

        # Define the RERUN_SDK_COMPILED_AS_SHARED_LIBRARY if building shared on Windows
        if self.settings.os == "Windows" and self.options.shared:
            self.cpp_info.defines.append("RERUN_SDK_COMPILED_AS_SHARED_LIBRARY")

        # TODO: Consider components if you want finer control (e.g., rerun::sdk, rerun::c)
        # self.cpp_info.components["sdk"].libs = ["rerun_sdk"]
        # self.cpp_info.components["sdk"].requires = ["arrow::arrow", "c"]
        # self.cpp_info.components["c"].libs = ["rerun_c"]
        # ... add system libs/frameworks to component "c" ...

    def export(self):
        # Copy rerun_c sources into the export folder to make them available in the cache
        rerun_c_base = os.path.join(self.recipe_folder, "..", "crates", "top", "rerun_c")
        # Destination path within the export folder
        export_dest = os.path.join(self.export_folder, "_deps", "rerun_c")

        copy(self, "Cargo.toml", src=rerun_c_base, dst=export_dest)
        copy(self, "src/*", src=os.path.join(rerun_c_base, "src"), dst=os.path.join(export_dest, "src"))

        # Create include directory in the export folder to store the rerun.h file
        export_include_dir = os.path.join(export_dest, "include")
        if not os.path.exists(export_include_dir):
            os.makedirs(export_include_dir)

        # Copy rerun.h from the rerun_cpp src directory to the include directory in the export folder
        rerun_h_src = os.path.join(self.recipe_folder, "src", "rerun", "c", "rerun.h")
        if os.path.exists(rerun_h_src):
            copy(self, "rerun.h", src=os.path.dirname(rerun_h_src), dst=export_include_dir)
        else:
            self.output.warning(f"rerun.h not found at {rerun_h_src}")
