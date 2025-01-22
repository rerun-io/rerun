from __future__ import annotations

import os
import subprocess

# These files are allowed to be larger than our limit
FILES_ALLOWED_TO_BE_LARGE = {
    "CHANGELOG.md",
    "Cargo.lock",
    "crates/build/re_types_builder/src/codegen/cpp/mod.rs",  # TODO(andreas, #7245): This file should really be split up. Might get shorter after removing non-eager serialization path.
    "crates/build/re_types_builder/src/reflection.rs",
    "crates/store/re_dataframe/src/query.rs",
    "crates/store/re_types/src/datatypes/tensor_buffer.rs",
    "crates/store/re_types/src/reflection/mod.rs",
    "crates/top/re_sdk/src/recording_stream.rs",
    "crates/viewer/re_ui/data/Inter-Medium.otf",
    "docs/snippets/INDEX.md",
    "pixi.lock",
    "rerun_cpp/docs/Doxyfile",
}

# Paths with the following prefixes are allowed to contain PNG files that are not checked into LFS
PATH_PREFIXES_ALLOWED_TO_CONTAIN_NON_LFS_PNGS = (
    "crates/viewer/re_ui/data/icons/",
    "crates/viewer/re_ui/data/logo_dark_mode.png",
    "crates/viewer/re_ui/data/logo_light_mode.png",
    "crates/viewer/re_viewer/data/app_icon_mac.png",
    "crates/viewer/re_viewer/data/app_icon_windows.png",
    "docs/snippets/all/archetypes/ferris.png",
    "docs/snippets/src/snippets/ferris.png",
    "examples/assets/example.png",
)


def check_large_files(files_to_check: set[str]) -> int:
    """Check for files that are too large to be checked into the repository."""

    maximum_size = 100 * 1024

    result = 0
    for file_path in files_to_check:
        actual_size = os.path.getsize(file_path)

        if actual_size >= maximum_size:
            if file_path not in FILES_ALLOWED_TO_BE_LARGE:
                print(f"{file_path} is {actual_size} bytes (max allowed is {maximum_size} bytes)")
                result = 1

    print(f"checked {len(files_to_check)} files")

    return result


def check_for_non_lfs_pngs(files_to_check: set[str]) -> int:
    """Check for PNG files that are not checked into LFS."""

    result = 0
    for file_path in files_to_check:
        if file_path.startswith(PATH_PREFIXES_ALLOWED_TO_CONTAIN_NON_LFS_PNGS):
            continue

        print(f"{file_path} is a PNG file that is not checked into LFS")
        result = 1

    print(f"checked {len(files_to_check)} pngs")

    return result


def main() -> None:
    script_path = os.path.dirname(os.path.realpath(__file__))
    os.chdir(os.path.join(script_path, "../.."))

    all_tracked_files = set(subprocess.check_output(["git", "ls-files"]).decode().splitlines())
    lfs_files = set(subprocess.check_output(["git", "lfs", "ls-files", "-n"]).decode().splitlines())
    not_lfs_files = all_tracked_files - lfs_files

    result = check_large_files(not_lfs_files)
    if result != 0:
        exit(result)

    all_tracked_pngs = {f for f in all_tracked_files if f.endswith(".png")}
    not_lfs_pngs = all_tracked_pngs - lfs_files

    result = check_for_non_lfs_pngs(not_lfs_pngs)
    if result != 0:
        exit(result)


if __name__ == "__main__":
    main()
