from __future__ import annotations

import os
import subprocess
import sys

# These files are allowed to be larger than our limit
FILES_ALLOWED_TO_BE_LARGE = {
    "Cargo.lock",
    "CHANGELOG.md",
    "crates/build/re_types_builder/src/codegen/cpp/mod.rs",  # TODO(andreas): This file should really be split up.
    "crates/build/re_types_builder/src/codegen/python/mod.rs",
    "crates/store_app/re_dataframe/src/query.rs",
    "crates/store/re_protos/src/v1alpha1/rerun.cloud.v1alpha1.rs",
    "crates/store/re_protos/src/v1alpha1/rerun.cloud.v1alpha1.ext.rs",
    "crates/store/re_protos/src/v1alpha1/rerun.viewer_control.v1alpha1.rs",
    "crates/store_app/re_query/src/range_zip/generated.rs",
    "crates/store/re_sdk_types/src/encodings/tensor_buffer.rs",
    "crates/store/re_sdk_types/src/reflection/mod.rs",
    "crates/top/re_sdk/src/recording_stream.rs",
    "crates/top/re_viewer/src/app/viewer_control/mod.rs",
    "crates/ui_tools/re_ui/data/Inter-Variable.ttf",
    "crates/ui_tools/re_ui/data/JetBrainsMono-Variable.ttf",
    "crates/top/re_viewer/data/app_icon_mac.png",
    "docs/snippets/INDEX.md",
    "pixi.lock",
    "rerun_cpp/docs/Doxyfile",
    "rerun_js/web-viewer/index.ts",
    "uv.lock",
    # Examples excluded from the uv workspace so they maintain standalone lockfiles.
    "examples/python/dataloader/uv.lock",
    "examples/python/droid_semantic_search/uv.lock",
}

# Paths with the following prefixes are allowed to contain PNG files that are not checked into LFS
PATH_PREFIXES_ALLOWED_TO_CONTAIN_NON_LFS_PNGS = (
    "crates/ui_tools/re_ui/data/icons/",
    "crates/ui_tools/re_ui/data/logo_dark_mode.png",
    "crates/ui_tools/re_ui/data/logo_light_mode.png",
    "crates/top/re_viewer/data/app_icon.png",
    "crates/top/re_viewer/data/app_icon_mac.png",
    "crates/top/re_web_viewer_server/web_viewer/apple-touch-icon.png",
    "docs/snippets/all/archetypes/ferris.png",
    "docs/snippets/src/snippets/ferris.png",
    "examples/assets/example.png",
)


DEFAULT_MAXIMUM_SIZE = 100 * 1024

# Insta snapshots can contain entire tables of Arrow record batches.
MAXIMUM_SIZE_PER_EXTENSION = {
    ".snap": 2 * 1024 * 1024,
}


def check_large_files(directory: str, files_to_check: set[str], files_allowed_to_be_large: set[str]) -> int:
    """Check for files that are too large to be checked into the repository."""

    result = 0
    for file_path in files_to_check:
        full_path = os.path.join(directory, file_path)
        if not os.path.exists(full_path):
            continue

        actual_size = os.path.getsize(full_path)

        _, ext = os.path.splitext(file_path)
        maximum_size = MAXIMUM_SIZE_PER_EXTENSION.get(ext, DEFAULT_MAXIMUM_SIZE)

        if actual_size >= maximum_size:
            if file_path not in files_allowed_to_be_large:
                print(f"{file_path} is {actual_size} bytes (max allowed is {maximum_size} bytes)")
                result = 1

    print(f"checked {len(files_to_check)} files")

    return result


def check_for_non_lfs_pngs(files_to_check: set[str], path_prefixes_allowed: tuple[str, ...]) -> int:
    """Check for PNG files that are not checked into LFS."""

    result = 0
    for file_path in files_to_check:
        if file_path.startswith(path_prefixes_allowed):
            continue

        print(f"{file_path} is a PNG file that is not checked into LFS")
        result = 1

    print(f"checked {len(files_to_check)} pngs")

    return result


def check_directory(
    directory: str,
    files_allowed_to_be_large: set[str],
    path_prefixes_allowed_to_contain_non_lfs_pngs: tuple[str, ...],
) -> int:
    """Check tracked files outside LFS in `directory` and return a non-zero error code if a check fails.

    Paths in the allowed sets are relative to `directory`.
    `directory` can be the root of the git repository or a folder inside it.
    Repositories that contain this one as a folder call this with their own allowed paths.
    """

    def git(*args: str) -> str:
        return subprocess.check_output(["git", *args], cwd=directory).decode()

    # `git ls-files --full-name` and `git lfs ls-files` return paths relative to the repository root.
    # `-z` turns off the quoting of non-ASCII paths. `git lfs ls-files` never quotes them.
    prefix = git("rev-parse", "--show-prefix").strip()
    all_tracked_files = set(git("ls-files", "-z", "--full-name").split("\0")) - {""}
    lfs_files = set(git("lfs", "ls-files", "-n").splitlines())

    not_lfs_files = {f.removeprefix(prefix) for f in all_tracked_files - lfs_files}
    not_lfs_pngs = {f for f in not_lfs_files if f.endswith(".png")}

    large_files_result = check_large_files(directory, not_lfs_files, files_allowed_to_be_large)
    pngs_result = check_for_non_lfs_pngs(not_lfs_pngs, path_prefixes_allowed_to_contain_non_lfs_pngs)
    return large_files_result or pngs_result


def main() -> None:
    script_path = os.path.dirname(os.path.realpath(__file__))
    result = check_directory(
        os.path.join(script_path, "../.."),
        FILES_ALLOWED_TO_BE_LARGE,
        PATH_PREFIXES_ALLOWED_TO_CONTAIN_NON_LFS_PNGS,
    )
    sys.exit(result)


if __name__ == "__main__":
    main()
