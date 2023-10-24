"""
Script to update a Github release's assets.

Given a Github release ID (e.g. `prerelease` or `0.9.0`), this script will fetch the associated
binary assets from our cloud storage (`build.rerun.io`) and upload them to the release as
native assets.

This is expected to be run by the release & pre-release workflows.

You can also run it manually if you want to update a specific release's assets:
  python scripts/ci/sync_release_assets.py --github-release prerelease --github-token <token> --update

Requires the following packages:
  pip install google-cloud-storage PyGithub
"""
from __future__ import annotations

import argparse
from typing import Dict

from github import Github
from github.GitRelease import GitRelease
from google.cloud import storage

Assets = Dict[str, storage.Blob]


def fetch_binary_assets(
    tag: str,
    commit: str,
    *,
    do_wheels: bool = True,
    do_rerun_c: bool = True,
    do_rerun_cpp_sdk: bool = True,
    do_rerun_cli: bool = True,
) -> Assets:
    """Given a release ID, fetches all associated binary assets from our cloud storage (build.rerun.io)."""
    assets = dict()

    gcs = storage.Client()
    bucket = gcs.bucket("rerun-builds")

    commit_short = commit[:7]
    print(f"Fetching binary assets for #{commit_short}…")
    print(f"  - wheels: {do_wheels}")
    print(f"  - C libs: {do_rerun_c}")
    print(f"  - C++ uber SDK: {do_rerun_cpp_sdk}")
    print(f"  - CLI (Viewer): {do_rerun_cli}")

    # Python wheels
    if do_wheels:
        wheel_blobs = list(bucket.list_blobs(prefix=f"commit/{commit_short}/wheels"))
        for blob in [bucket.get_blob(blob.name) for blob in wheel_blobs if blob.name.endswith(".whl")]:
            if blob is not None and blob.name is not None:
                name = blob.name.split("/")[-1]

                if "macosx" in name:
                    if "x86_64" in name:
                        name = f"rerun_sdk-{tag}-aarch64-apple-darwin.whl"
                    if "arm64" in name:
                        name = f"rerun_sdk-{tag}-x86_64-apple-darwin.whl"

                if "manylinux_2_31_x86_64" in name:
                    if "x86_64" in name:
                        name = f"rerun_sdk-{tag}-x86_64-unknown-linux-gnu.whl"

                if "win_amd64" in name:
                    name = f"rerun_sdk-{tag}-x86_64-pc-windows-msvc.whl"

                print(f"    Found Python wheel: {name} ")
                assets[name] = blob

    # rerun_c
    if do_rerun_c:
        rerun_c_blobs = [
            (
                f"rerun_c-{tag}-x86_64-pc-windows-msvc.lib",
                bucket.get_blob(f"commit/{commit_short}/rerun_c/windows/rerun_c.lib"),
            ),
            (
                f"librerun_c-{tag}-x86_64-unknown-linux-gnu.a",
                bucket.get_blob(f"commit/{commit_short}/rerun_c/linux/librerun_c.a"),
            ),
            (
                f"librerun_c-{tag}-aarch64-apple-darwin.a",
                bucket.get_blob(f"commit/{commit_short}/rerun_c/macos-arm/librerun_c.a"),
            ),
            (
                f"librerun_c-{tag}-x86_64-apple-darwin.a",
                bucket.get_blob(f"commit/{commit_short}/rerun_c/macos-intel/librerun_c.a"),
            ),
        ]
        for name, blob in rerun_c_blobs:
            if blob is not None:
                print(f"    Found Rerun C library: {name}")
                assets[name] = blob

    # rerun_cpp_sdk
    if do_rerun_cpp_sdk:
        rerun_cpp_sdk_blob = bucket.get_blob(f"commit/{commit_short}/rerun_cpp_sdk.zip")
        for blob in [rerun_cpp_sdk_blob]:
            if blob is not None and blob.name is not None:
                name = blob.name.split("/")[-1]
                print(f"    Found Rerun cross-platform bundle: {name}")
                assets[name] = blob
                # NOTE: Want a versioned one too.
                assets[f"rerun_cpp_sdk-{tag}-multiplatform.zip"] = blob

    # rerun-cli
    if do_rerun_cli:
        rerun_cli_blobs = [
            (
                f"rerun-cli-{tag}-x86_64-pc-windows-msvc.exe",
                bucket.get_blob(f"commit/{commit_short}/rerun-cli/windows/rerun.exe"),
            ),
            (
                f"rerun-cli-{tag}-x86_64-unknown-linux-gnu",
                bucket.get_blob(f"commit/{commit_short}/rerun-cli/linux/rerun"),
            ),
            (
                f"rerun-cli-{tag}-aarch64-apple-darwin",
                bucket.get_blob(f"commit/{commit_short}/rerun-cli/macos-arm/rerun"),
            ),
            (
                f"rerun-cli-{tag}-x86_64-apple-darwin",
                bucket.get_blob(f"commit/{commit_short}/rerun-cli/macos-intel/rerun"),
            ),
        ]
        for name, blob in rerun_cli_blobs:
            if blob is not None:
                print(f"    Found Rerun CLI binary: {name}")
                assets[name] = blob

    return assets


def remove_release_assets(release: GitRelease):
    print("Removing pre-existing release assets…")

    for asset in release.get_assets():
        print(f"    Removing {asset.name}…")
        asset.delete_asset()


def update_release_assets(release: GitRelease, assets: Assets):
    print("Updating release assets…")

    for name, blob in assets.items():
        blob_contents = blob.download_as_bytes()
        # NOTE: Do _not_ ever use `blob.size`, it might or might not give you the size you expect
        # depending on the versions of your gcloud dependencies, which in turn might or might not fail
        # the upload in all kinds of unexpected ways (including SSL errors!) depending on the versions
        # of your reqwest & pygithub dependencies.
        blob_raw_size = len(blob_contents)
        print(f"    Uploading {name} ({blob_raw_size} bytes)…")
        release.upload_asset_from_memory(
            blob_contents,
            blob_raw_size,
            name,
            content_type="application/octet-stream",
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", default="rerun-io/rerun", help="GitHub repository")
    parser.add_argument(
        "--github-release", required=True, help="ID of the Github (pre)release (e.g. `prerelease` or `0.9.0`)"
    )
    parser.add_argument("--github-timeout", default=120, help="Timeout for Github related operations")
    parser.add_argument("--remove", action="store_true", help="Remove existing assets from the specified release")
    parser.add_argument("--update", action="store_true", help="Update new assets to the specified release")
    parser.add_argument("--no-wheels", action="store_true", help="Don't upload Python wheels")
    parser.add_argument("--no-rerun-c", action="store_true", help="Don't upload C libraries")
    parser.add_argument("--no-rerun-cpp-sdk", action="store_true", help="Don't upload C++ uber SDK")
    parser.add_argument("--no-rerun-cli", action="store_true", help="Don't upload CLI")
    args = parser.parse_args()

    gh = Github(args.github_token, timeout=args.github_timeout)
    repo = gh.get_repo(args.github_repository)
    release = repo.get_release(args.github_release)
    commit = dict([(tag.name, tag.commit) for tag in repo.get_tags()])[args.github_release]

    print(
        f'Syncing binary assets for release `{release.tag_name}` ("{release.title}" @{release.published_at}) #{commit.sha[:7]}…'
    )

    assets = fetch_binary_assets(
        release.tag_name,
        commit.sha,
        do_wheels=not args.no_wheels,
        do_rerun_c=not args.no_rerun_c,
        do_rerun_cpp_sdk=not args.no_rerun_cpp_sdk,
        do_rerun_cli=not args.no_rerun_cli,
    )

    if args.remove:
        remove_release_assets(release)

    if args.update:
        update_release_assets(release, assets)


if __name__ == "__main__":
    main()
