from conan import ConanFile
from conan.tools.files import get, copy
from conan.tools.cmake import cmake_layout, CMakeToolchain, CMakeDeps, CMake
from conan.errors import ConanException
import os, glob
import re

class RerunCppSdkConan(ConanFile):
    name            = "rerun_cpp_sdk"
    license         = "Apache-2.0"
    url             = "https://github.com/rerun-io/rerun"
    description     = "Rerun C++ SDK with embedded Rust C core"
    settings        = "os", "arch", "compiler", "build_type"
    options         = {"shared": [True, False]}
    default_options = {"shared": False}
    requires        = ["arrow/15.0.0"]
    exports_sources = "lib/*", "src/*", "CMakeLists.txt"
    no_copy_source  = False

    def layout(self):
        cmake_layout(self)

    def set_version(self):
        # Read version from sdk_info.h
        sdk_info_path = os.path.join(self.recipe_folder, "src", "rerun", "c", "sdk_info.h")
        if os.path.exists(sdk_info_path):
            with open(sdk_info_path, "r") as f:
                content = f.read()
                version_match = re.search(r'#define\s+RERUN_SDK_HEADER_VERSION\s+"([^"]+)"', content)
                if version_match:
                    self.version = version_match.group(1)
                else:
                    raise ConanException("Could not find RERUN_SDK_HEADER_VERSION in sdk_info.h")
        else:
            raise ConanException("Could not find sdk_info.h at path: " + sdk_info_path)

    def source(self):
        sdk_url = (
            f"https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip"
        )
        get(self, sdk_url, strip_root=True)

    def generate(self):
        tc = CMakeToolchain(self)
        tc.cache_variables["RERUN_DOWNLOAD_AND_BUILD_ARROW"] = False
        tc.cache_variables["RERUN_ARROW_LINK_SHARED"]       = False
        tc.cache_variables["RERUN_INSTALL_RERUN_C"]         = True
        tc.cache_variables["RERUN_C_LIB"] = os.path.join(
            self.source_folder, "lib", self._c_lib_filename()
        )
        tc.generate()

        deps = CMakeDeps(self)
        deps.generate()

    def _c_lib_filename(self):
        mapping = {
            "Linux":  {"x86_64": "librerun_c__linux_x64.a",  "armv8": "librerun_c__linux_arm64.a"},
            "Macos":  {"x86_64": "librerun_c__macos_x64.a",  "armv8": "librerun_c__macos_arm64.a"},
            "Windows":{"x86_64": "rerun_c__win_x64.lib"},
        }
        os_  = str(self.settings.os)
        arch = str(self.settings.arch)
        try:
            return mapping[os_][arch]
        except KeyError:
            raise ConanException(f"No rerun_c binary for {os_}/{arch}")

    def build(self):
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        # 1) CMake install
        cmake = CMake(self)
        cmake.install()

        # 2) Copy & rename the shipped C-core into a standard name
        libdir = os.path.join(self.package_folder, "lib")
        c_filename = self._c_lib_filename()
        # copy the exactly-matching file:
        copy(self, c_filename,
             src=os.path.join(self.source_folder, "lib"),
             dst=libdir,
             keep_path=False)

        # now rename it to the standard name
        if self.settings.os == "Windows":
            new_name = "rerun_c.lib"
        else:
            new_name = "librerun_c.a"
        old_path = os.path.join(libdir, c_filename)
        new_path = os.path.join(libdir, new_name)
        os.replace(old_path, new_path)

        # 3) Copy headers into include/
        copy(self, "*.hpp",
             src=os.path.join(self.source_folder, "src"),
             dst=os.path.join(self.package_folder, "include"))
        copy(self, "*.h",
             src=os.path.join(self.source_folder, "src"),
             dst=os.path.join(self.package_folder, "include"))

    def package_info(self):
        # C-core component
        self.cpp_info.components["c_core"].libs = ["rerun_c"]
        # C++ SDK component
        self.cpp_info.components["rerun_sdk"].libs     = ["rerun_sdk"]
        self.cpp_info.components["rerun_sdk"].requires = ["c_core", "arrow::arrow"]

        # Make sure `find_package(rerun_sdk)` works
        self.cpp_info.set_property("cmake_file_name",   "rerun_sdk")
        self.cpp_info.set_property("cmake_target_name", "rerun_sdk")
