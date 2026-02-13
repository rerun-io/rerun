"""
Script to update a Github release's assets.

Given a Github release ID (e.g. `prerelease` or `0.9.0`), this script will fetch the associated
binary assets from our cloud storage (`build.rerun.io`) and upload them to the release as
native assets.

This is expected to be run by the release & pre-release workflows.

You can also run it manually if you want to update a specific release's assets:
  python scripts/ci/sync_release_assets.py --github-release prerelease --github-token <token> --update
"""

from __future__ import annotations

import argparse
import sys
import time
from typing import TYPE_CHECKING, cast

from github import Github
from google.cloud import storage

if TYPE_CHECKING:
    from github.GitRelease import GitRelease
    from github.Repository import Repository

Assets = dict[str, storage.Blob]


def get_any_release(repo: Repository, tag_name: str) -> GitRelease | None:
    """Fetch any release from a Github repository, even drafts."""
    # Do _not_ use `repo.get_release`, it silently ignores drafts.
    return next(rel for rel in repo.get_releases() if rel.tag_name == tag_name)


def fetch_binary_assets(
    tag: str,
    commit: str,
    *,
    do_wheels: bool = True,
    do_rerun_c: bool = True,
    do_rerun_cpp_sdk: bool = True,
    do_rerun_cli: bool = True,
    do_rerun_js: bool = True,
) -> Assets:
    """Given a release ID, fetches all associated binary assets from our cloud storage (build.rerun.io)."""
    assets = {}

    gcs = storage.Client()
    bucket = gcs.bucket("rerun-builds")

    commit_short = commit[:7]
    print(f"Fetching the following binary assets for #{commit_short}:")
    if do_wheels:
        print("  - Python wheels")
    if do_rerun_c:
        print("  - C libs")
    if do_rerun_cpp_sdk:
        print("  - C++ uber SDK")
    if do_rerun_cli:
        print("  - CLI (Viewer)")
    if do_rerun_js:
        print("  - JS package")

    all_found = True

    # Python wheels
    if do_wheels:
        found = False
        wheel_blobs = list(bucket.list_blobs(prefix=f"commit/{commit_short}/wheels"))
        for blob in (bucket.get_blob(blob.name) for blob in wheel_blobs if blob.name.endswith(".whl")):
            if blob is not None and blob.name is not None:
                name = blob.name.split("/")[-1]

                # NOTE(cmc): I would love to rename those so they match the versioning of our
                # other assets, but that breaks `pip install`…
                # if "macosx" in name:
                #     if "x86_64" in name:
                #         name = f"rerun_sdk-{tag}-aarch64-apple-darwin.whl"
                #     if "arm64" in name:
                #         name = f"rerun_sdk-{tag}-x86_64-apple-darwin.whl"
                #
                # if "manylinux_2_28_x86_64" in name:
                #     if "x86_64" in name:
                #         name = f"rerun_sdk-{tag}-x86_64-unknown-linux-gnu.whl"
                #
                # if "win_amd64" in name:
                #     name = f"rerun_sdk-{tag}-x86_64-pc-windows-msvc.whl"
                print(f"Found Python wheels: {name}")
                found = True
                assets[name] = blob

        if not found:
            all_found = False
            print("Python wheels not found")

    # rerun_c
    if do_rerun_c:
        rerun_c_blobs = [
            (
                f"rerun_c-{tag}-x86_64-pc-windows-msvc.lib",
                f"commit/{commit_short}/rerun_c/windows-x64/rerun_c.lib",
            ),
            (
                f"librerun_c-{tag}-x86_64-unknown-linux-gnu.a",
                f"commit/{commit_short}/rerun_c/linux-x64/librerun_c.a",
            ),
            (
                f"librerun_c-{tag}-aarch64-unknown-linux-gnu.a",
                f"commit/{commit_short}/rerun_c/linux-arm64/librerun_c.a",
            ),
            (
                f"librerun_c-{tag}-aarch64-apple-darwin.a",
                f"commit/{commit_short}/rerun_c/macos-arm64/librerun_c.a",
            ),
        ]
        for name, blob_url in rerun_c_blobs:
            blob = bucket.get_blob(blob_url)
            if blob is not None:
                print(f"Found Rerun C library: {name}")
                assets[name] = blob
            else:
                all_found = False
                print(f"Failed to fetch blob {blob_url} ({name})")

    # rerun_cpp_sdk
    if do_rerun_cpp_sdk:
        rerun_cpp_sdk_blob = bucket.get_blob(f"commit/{commit_short}/rerun_cpp_sdk.zip")
        for blob in [rerun_cpp_sdk_blob]:
            if blob is not None and blob.name is not None:
                name = blob.name.split("/")[-1]
                print(f"Found Rerun cross-platform bundle: {name}")
                assets[f"rerun_cpp_sdk-{tag}-multiplatform.zip"] = blob

                # Upload again as rerun_cpp_sdk.zip for convenience.
                #
                # ATTENTION: Renaming this file has tremendous ripple effects:
                # Not only is this the convenient short name we use in examples,
                # we also rely on https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip
                # to always give you the latest stable version of the Rerun SDK.
                # -> The name should *not* contain the version number.
                assets["rerun_cpp_sdk.zip"] = blob
            else:
                all_found = False
                print("Rerun cross-platform bundle not found")

    # rerun-cli
    if do_rerun_cli:
        rerun_cli_blobs = [
            (
                f"rerun-cli-{tag}-x86_64-pc-windows-msvc.exe",
                f"commit/{commit_short}/rerun-cli/windows-x64/rerun.exe",
            ),
            (
                f"rerun-cli-{tag}-x86_64-unknown-linux-gnu",
                f"commit/{commit_short}/rerun-cli/linux-x64/rerun",
            ),
            (
                f"rerun-cli-{tag}-aarch64-unknown-linux-gnu",
                f"commit/{commit_short}/rerun-cli/linux-arm64/rerun",
            ),
            (
                f"rerun-cli-{tag}-aarch64-apple-darwin",
                f"commit/{commit_short}/rerun-cli/macos-arm64/rerun",
            ),
        ]
        for name, blob_url in rerun_cli_blobs:
            blob = bucket.get_blob(blob_url)
            if blob is not None:
                print(f"Found Rerun CLI binary: {name}")
                assets[name] = blob
            else:
                all_found = False
                print(f"Failed to fetch blob {blob_url} ({name})")

    # rerun-js
    if do_rerun_js:
        # note: we don't include the version tag in the asset name here,
        #       otherwise `latest` downloads contain the version number.
        rerun_js_blobs = [
            (
                "rerun-js-web-viewer.tar.gz",
                f"commit/{commit_short}/rerun_js/web-viewer.tar.gz",
            ),
            (
                "rerun-js-web-viewer-react.tar.gz",
                f"commit/{commit_short}/rerun_js/web-viewer-react.tar.gz",
            ),
        ]
        for name, blob_url in rerun_js_blobs:
            blob = bucket.get_blob(blob_url)
            if blob is not None:
                print(f"Found Rerun JS package: {name}")
                assets[name] = blob
            else:
                all_found = False
                print(f"Failed to fetch blob {blob_url} ({name})")

    if not all_found:
        raise Exception("Some requested assets were not found")

    return assets


def remove_release_assets(release: GitRelease) -> None:
    print("Removing pre-existing release assets…")

    for asset in release.get_assets():
        print(f"Removing {asset.name}…")
        asset.delete_asset()


def update_release_assets(release: GitRelease, assets: Assets) -> None:
    print("Updating release assets…")

    for name, blob in assets.items():
        blob_contents = blob.download_as_bytes()
        # NOTE: Do _not_ ever use `blob.size`, it might or might not give you the size you expect
        # depending on the versions of your gcloud dependencies, which in turn might or might not fail
        # the upload in all kinds of unexpected ways (including SSL errors!) depending on the versions
        # of your reqwest & pygithub dependencies.
        blob_raw_size = len(blob_contents)
        print(f"Uploading {name} ({blob_raw_size} bytes)…")
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
        "--github-release",
        required=True,
        help="Github (pre)release tag (e.g. `prerelease` or `0.9.0`)",
    )
    parser.add_argument("--github-timeout", default=120, help="Timeout for Github related operations")
    parser.add_argument("--wait", default=0, help="Sleep a bit before doing anything")
    parser.add_argument(
        "--remove",
        action="store_true",
        help="Remove existing assets from the specified release",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help="Update new assets to the specified release",
    )
    parser.add_argument("--no-wheels", action="store_true", help="Don't upload Python wheels")
    parser.add_argument("--no-rerun-c", action="store_true", help="Don't upload C libraries")
    parser.add_argument("--no-rerun-cpp-sdk", action="store_true", help="Don't upload C++ uber SDK")
    parser.add_argument("--no-rerun-cli", action="store_true", help="Don't upload CLI")
    parser.add_argument("--no-rerun-js", action="store_true", help="Don't upload JS package")
    args = parser.parse_args()

    # Wait for a bit before doing anything, if you must.
    wait_time_secs = float(args.wait)
    if wait_time_secs > 0.0:
        print(f"Waiting for {wait_time_secs}s…")
        time.sleep(wait_time_secs)

    gh = Github(args.github_token, timeout=args.github_timeout)
    repo = gh.get_repo(args.github_repository)
    release = cast("GitRelease", get_any_release(repo, args.github_release))
    commit = {tag.name: tag.commit for tag in repo.get_tags()}[args.github_release]

    if release.body is None:
        print(
            "Release has no body - make sure to add release notes! "
            "You might also run into this if you created the release manually "
            "and not from the draft created by the release job, please check.",
            file=sys.stderr,
        )
        sys.exit(1)

    print(
        f'Syncing binary assets for release `{release.tag_name}` ("{release.title}" @{release.published_at} draft={release.draft}) #{commit.sha[:7]}…',
    )

    assets = fetch_binary_assets(
        release.tag_name,
        commit.sha,
        do_wheels=not args.no_wheels,
        do_rerun_c=not args.no_rerun_c,
        do_rerun_cpp_sdk=not args.no_rerun_cpp_sdk,
        do_rerun_cli=not args.no_rerun_cli,
        do_rerun_js=not args.no_rerun_js,
    )

    if args.remove:
        remove_release_assets(release)

    if args.update:
        update_release_assets(release, assets)

    # Github will unconditionally draft a release in some cases (e.g. because the branch it has
    # originated from has been modified since). This is beyond our control.
    #
    # Draft releases are not accessible through any of the expected ways, so we make sure to fix
    # that.
    #
    # See e.g. <https://github.com/ncipollo/release-action/issues/317>.
    if release.draft:
        print("Detected mistakenly drafted release, undrafting…")
        release.update_release(release.title, release.body, draft=False, prerelease=release.prerelease)


if __name__ == "__main__":
    main()
