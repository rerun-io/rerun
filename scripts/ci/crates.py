#!/usr/bin/env python3

"""
Versioning and packaging.

Use the script:
    pixi run python scripts/ci/crates.py --help

    # Update crate versions to the next prerelease version,
    # e.g. `0.8.0` -> `0.8.0-alpha.0`, `0.8.0-alpha.0` -> `0.8.0-alpha.1`
    pixi run python scripts/ci/crates.py version --bump prerelease --dry-run

    # Update crate versions to an exact version
    pixi run python scripts/ci/crates.py version --exact 0.10.1 --dry-run

    # Publish all crates in topological order
    pixi run python scripts/ci/publish.py --token <CRATES_IO_TOKEN>
"""

from __future__ import annotations

import argparse
import os.path
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from enum import Enum
from glob import glob
from multiprocessing import cpu_count
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import git
import requests
import tomlkit
from colorama import Fore, init as colorama_init
from dag import DAG, RateLimiter
from semver import VersionInfo

if TYPE_CHECKING:
    from collections.abc import Generator

CARGO_PATH = shutil.which("cargo") or "cargo"
DEFAULT_PRE_ID = "alpha"
MAX_PUBLISH_WORKERS = 3

R = Fore.RED
G = Fore.GREEN
B = Fore.BLUE
X = Fore.RESET


def cargo(
    args: str,
    *,
    cargo_version: str | None = None,
    cwd: str | Path | None = None,
    env: dict[str, Any] | None = None,
    dry_run: bool = False,
    capture: bool = False,
) -> Any:
    if env is None:
        env = {}

    if cargo_version is None:
        cmd = [CARGO_PATH, *args.split()]
    else:
        cmd = [CARGO_PATH, f"+{cargo_version}", *args.split()]

    # Create and print a sanitized copy of the command
    sanitized_cmd = cmd.copy()
    try:
        token_idx = sanitized_cmd.index("--token")
        if token_idx + 1 < len(sanitized_cmd):
            sanitized_cmd[token_idx + 1] = "***"
    except ValueError:
        pass
    print(f"> CWD={cwd} cargo {subprocess.list2cmdline(sanitized_cmd[1:])}")

    if not dry_run:
        stderr = subprocess.STDOUT if capture else None
        subprocess.check_output(cmd, cwd=cwd, env=env, stderr=stderr)


class Crate:
    def __init__(self, manifest: dict[str, Any], path: Path) -> None:
        self.manifest = manifest
        self.path = path

    def __str__(self) -> str:
        return f"{self.manifest['package']['name']}@{self.path}"


def get_workspace_crates(root: dict[str, Any]) -> dict[str, Crate]:
    """
    Returns a dictionary of workspace crates.

    The crates are in the same order as they appear in the root `Cargo.toml`
    under `workspace.members`.
    """

    crates: dict[str, Crate] = {}
    for pattern in root["workspace"]["members"]:
        for crate in [member for member in glob(pattern) if os.path.isdir(member)]:
            crate_path = Path(crate)
            crate_cargo_toml = crate_path / "Cargo.toml"
            if not crate_cargo_toml.exists():
                continue
            manifest_text = crate_cargo_toml.read_text()
            manifest: dict[str, Any] = tomlkit.parse(manifest_text)
            crates[manifest["package"]["name"]] = Crate(manifest, crate_path)
    return crates


class DependencyKind(Enum):
    DIRECT = "direct"
    DEV = "dev"
    BUILD = "build"

    def manifest_key(self) -> str:
        if self.value == "direct":
            return "dependencies"
        else:
            return f"{self.value}-dependencies"


class Dependency:
    def __init__(self, name: str, manifest_key: list[str], kind: DependencyKind) -> None:
        self.name = name
        self.manifest_key = manifest_key
        self.kind = kind

    def get_info_in_manifest(self, manifest: dict[str, Any]) -> Any:
        info = manifest
        for key in self.manifest_key:
            info = info[key]
        return info


def crate_deps(member: dict[str, dict[str, Any]]) -> Generator[Dependency, None, None]:
    def get_deps_in(d: dict[str, dict[str, Any]], base_key: list[str]) -> Generator[Dependency, None, None]:
        if "dependencies" in d:
            for v in d["dependencies"].keys():
                yield Dependency(v, [*base_key, "dependencies", v], DependencyKind.DIRECT)
        if "dev-dependencies" in d:
            for v in d["dev-dependencies"].keys():
                yield Dependency(v, [*base_key, "dev-dependencies", v], DependencyKind.DEV)
        if "build-dependencies" in d:
            for v in d["build-dependencies"].keys():
                yield Dependency(v, [*base_key, "build-dependencies", v], DependencyKind.BUILD)

    yield from get_deps_in(member, [])
    if "target" in member:
        for target in member["target"].keys():
            yield from get_deps_in(member["target"][target], ["target", target])


def get_sorted_publishable_crates(ctx: Context, crates: dict[str, Crate]) -> dict[str, Crate]:
    """
    Returns crates topologically sorted in publishing order.

    This also filters any crates which have `publish` set to `false`.
    """

    visited: dict[str, bool] = {}
    output: dict[str, Crate] = {}

    def helper(
        ctx: Context,
        crates: dict[str, Crate],
        name: str,
    ) -> None:
        nonlocal visited, output

        # Circular references are possible, so we must check for cycles before recursing.
        if name in visited:
            return
        else:
            visited[name] = True

        crate = crates[name]
        for dependency in crate_deps(crate.manifest):
            assert dependency.name != name, f"Crate {name} had itself as a dependency"
            if dependency.name not in crates:
                continue
            helper(ctx, crates, dependency.name)

        # Insert only after all dependencies have been traversed
        publish = crate.manifest["package"].get("publish")
        if publish is None:
            ctx.error(f"Crate {B}{name}{X} does not have {B}package.publish{X} set.")
            return

        if publish:
            output[name] = crate

    for name in crates.keys():
        helper(ctx, crates, name)
    return output


class Bump(Enum):
    MAJOR = "major"
    """Bump the major version, e.g. `0.9.0-alpha.5+dev` -> `1.0.0`"""

    MINOR = "minor"
    """Bump the minor version, e.g. `0.9.0-alpha.5+dev` -> `0.10.0`"""

    PATCH = "patch"
    """Bump the patch version, e.g. `0.9.0-alpha.5+dev` -> `0.9.1`"""

    PRERELEASE = "prerelease"
    """Bump the pre-release version, e.g. `0.9.0-alpha.5+dev` -> `0.9.0-alpha.6+dev`"""

    FINALIZE = "finalize"
    """Remove the pre-release identifier and build metadata, e.g. `0.9.0-alpha.5+dev` -> `0.9.0`"""

    AUTO = "auto"
    """
    Automatically determine the next version and bump to it.

    This depends on the latest version published to crates.io:
    - If it is a pre-release, then bump the pre-release.
    - If it is not a pre-release, then bump the minor version, and add `-alpha.N+dev`.
    """

    def __str__(self) -> str:
        return self.value

    def apply(self, version: VersionInfo, pre_id: str) -> VersionInfo:
        if self is Bump.MAJOR:
            return version.bump_major()
        elif self is Bump.MINOR:
            return version.bump_minor()
        elif self is Bump.PATCH:
            return version.bump_patch()
        elif self is Bump.PRERELEASE:
            if version.prerelease is not None and version.prerelease.split(".")[0] != pre_id:
                # reset the build number if the pre-id changes
                # e.g. by going from `alpha` to `rc`
                return version.finalize_version().bump_prerelease(token=pre_id)
            else:
                return version.bump_prerelease()
        elif self is Bump.FINALIZE:
            return version.finalize_version()
        elif self is Bump.AUTO:
            latest_version = get_version(Target.CratesIo)
            latest_version_finalized = latest_version.finalize_version()
            if latest_version == latest_version_finalized:
                # Latest published is not a pre-release, bump minor and add alpha+dev
                # example: 0.9.1 -> 0.10.0-alpha.1+dev
                return version.bump_minor().bump_prerelease(token="alpha").replace(build="dev")
            else:
                # Latest published is a pre-release, bump prerelease
                # example: 0.10.0-alpha.5 -> 0.10.0-alpha.6+dev
                return version.bump_prerelease(token="alpha").replace(build="dev")


def is_pinned(version: str) -> bool:
    return version.startswith("=")


class Context:
    ops: list[str] = []
    errors: list[str] = []

    def bump(self, path: str, prev: str, new: VersionInfo) -> None:
        # fmt: off
        op = " ".join([
            f"bump {B}{path}{X}",
            f"from {G}{prev}{X}",
            f"to {G}{new}{X}",
        ])
        # fmt: on
        self.ops.append(op)

    def publish(self, crate: str, version: str) -> None:
        # fmt: off
        op = " ".join([
            f"publish {B}{crate}{X}",
            f"version {G}{version}{X}",
        ])
        # fmt: on
        self.ops.append(op)

    def plan(self, operation: str) -> None:
        self.ops.append(operation)

    def error(self, *e: str) -> None:
        self.errors.append("\n".join(e))

    def finish(self) -> None:
        if len(self.errors) > 0:
            print("Encountered some errors:")
            for error in self.errors:
                print(error)
            sys.exit(1)
        else:
            print("The following operations will be performed:")
            for op in self.ops:
                print(op)
            print()


def bump_package_version(
    ctx: Context,
    crate: str,
    new_version: VersionInfo,
    manifest: dict[str, Any],
) -> None:
    if "package" in manifest and "version" in manifest["package"]:
        version = manifest["package"]["version"]
        if "workspace" not in version or not version["workspace"]:
            ctx.bump(crate, version, new_version)
            manifest["package"]["version"] = str(new_version)


def bump_dependency_versions(
    ctx: Context,
    crate: str,
    new_version: VersionInfo,
    manifest: dict[str, Any],
    crates: dict[str, Crate],
) -> None:
    # ensure `+metadata` is not included in dependency versions
    new_version = new_version.replace(build=None)
    for dependency in crate_deps(manifest):
        if dependency.name not in crates:
            continue

        info = dependency.get_info_in_manifest(manifest)
        if isinstance(info, str):
            ctx.error(
                f"{crate}.{dependency.name} should be specified as:",
                f'  {dependency.name} = {{ version = "' + info + '" }',
            )
        elif "version" in info:
            pin_prefix = "=" if new_version.prerelease is not None else ""
            update_to = pin_prefix + str(new_version)
            ctx.bump(
                f"{crate}.{dependency.name}",
                str(info["version"]),
                cast("VersionInfo", update_to),
            )
            info["version"] = update_to


def bump_version(dry_run: bool, bump: Bump | str | None, pre_id: str, dev: bool) -> None:
    ctx = Context()

    root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text(encoding="utf-8"))
    crates = get_workspace_crates(root)
    current_version = VersionInfo.parse(root["workspace"]["package"]["version"])

    new_version = current_version
    print(bump)
    if bump is not None:
        if isinstance(bump, Bump):
            new_version = bump.apply(new_version, pre_id)
        else:
            new_version = VersionInfo.parse(bump)
    if dev is not None:
        new_version = new_version.replace(build="dev" if dev else None)

    # There are a few places where versions are set:
    # 1. In the root `Cargo.toml` under `workspace.package.version`.
    bump_package_version(ctx, "(root)", new_version, root["workspace"])
    # 2. In the root `Cargo.toml` under `workspace.dependencies`,
    #    under the `{crate}.version` property.
    #    The version may be pinned by prefixing it with `=`.
    bump_dependency_versions(ctx, "(root)", new_version, root["workspace"], crates)

    for name, crate in crates.items():
        # 3. In the crate's `Cargo.toml` under `package.version`,
        #    although this may be set to `workspace=true`, in which case
        #    we don't bump it.
        bump_package_version(ctx, name, new_version, crate.manifest)
        # 4. In each crate's `Cargo.toml` under `dependencies`,
        #    `dev-dependencies`, and `build-dependencies`.
        #    Here the version may also be pinned by prefixing it with `=`.
        bump_dependency_versions(ctx, name, new_version, crate.manifest, crates)

    ctx.finish()

    # Save after bumping all versions
    if not dry_run:
        with Path("Cargo.toml").open("w", encoding="utf-8") as f:
            tomlkit.dump(root, f)
        for crate in crates.values():
            with Path(f"{crate.path}/Cargo.toml").open("w", encoding="utf-8") as f:
                tomlkit.dump(crate.manifest, f)
    cargo("update --workspace", dry_run=dry_run)
    if shutil.which("taplo") is not None:
        subprocess.check_output(["taplo", "fmt"])


def is_already_published(version: str, crate: Crate) -> bool:
    crate_name = crate.manifest["package"]["name"]
    resp = requests.get(
        f"https://crates.io/api/v1/crates/{crate_name}",
        headers={"user-agent": "rerun-publishing-script (rerun.io)"},
    )
    body = resp.json()

    # the request failed
    if not resp.ok:
        detail = body["errors"][0]["detail"]
        if resp.status_code == 404:
            return False  # New crate that hasn't been published before
        else:
            raise Exception(f"Failed to get crate '{crate_name}': {resp.status_code} {detail}")

    # crate has not been uploaded yet
    if "versions" not in body:
        return False

    # crate has been uploaded, check every version against what we're uploading
    versions: list[str] = [version["num"] for version in body["versions"]]
    for uploaded_version in versions:
        if uploaded_version == version:
            return True
    return False


def parse_retry_delay_secs(error_message: str) -> float | None:
    """Parses the retry-after datetime from a `cargo publish` error 429 message, and returns the seconds remaining until that time."""

    # Example:
    #   the remote server responded with an error (status 429 Too Many Requests):
    #   You have published too many crates in a short period of time.
    #   Please try again after Tue, 27 Dec 2022 17:25:13 GMT or email help@crates.io
    #   to have your limit increased.

    RETRY_AFTER_START = "Please try again after "
    RETRY_AFTER_END = " GMT or email help@crates.io"
    start = error_message.find(RETRY_AFTER_START)
    if start == -1:
        return None
    start += len(RETRY_AFTER_START)
    end = error_message.find(RETRY_AFTER_END, start)
    if end == -1:
        return None
    retry_after = datetime.strptime(error_message[start:end], "%a, %d %b %Y %H:%M:%S").replace(tzinfo=timezone.utc)
    return (retry_after - datetime.now(timezone.utc)).total_seconds() * MAX_PUBLISH_WORKERS


def publish_crate(crate: Crate, token: str, version: str, env: dict[str, Any], dry_run: bool) -> None:
    package = crate.manifest["package"]
    name = package["name"]

    # NOTE: `--quiet` here we run these in parallel
    publish_cmd = f"publish --quiet --locked --token {token}"
    if name == "re_web_viewer_server":
        # For some reason, cargo complains about the web viewer .wasm and .js being "dirty",
        # despite them being in .gitignore.
        # We don't know why. But we still want to publish:
        # TODO(#11199): remove this hack
        publish_cmd += " --allow-dirty"

    # This is a bit redundant given that we don't run the command in dry runs, but it allows setting `dry_run=False`
    # bellow for testing purposes.
    if dry_run:
        publish_cmd += " --dry-run"

    print(f"{G}Publishing{X} {B}{name}{X}…")
    retry_attempts = 5
    while True:
        try:
            cargo(
                publish_cmd,
                cwd=crate.path,
                env=env,
                dry_run=dry_run,
                capture=True,
            )

            if not dry_run and not is_already_published(version, crate):
                # Theoretically this shouldn't be needed… but sometimes it is.
                print(f"{R}Waiting for {name} to become available…")
                time.sleep(2)  # give crates.io some time to index the new crate
                num_retries = 0
                while not is_already_published(version, crate):
                    time.sleep(3)
                    num_retries += 1
                    if num_retries > 10:
                        print(f"{R}We published{X} {B}{name}{X} but it was never made available. Continuing anyway.")
                        return

            print(f"{G}Published{X} {B}{name}{X}@{B}{version}{X}")

            break
        except subprocess.CalledProcessError as e:
            error_message = e.stdout.decode("utf-8").strip()
            # if we get a 429, parse the retry delay from it
            # for any other error, retry after 6 seconds
            retry_delay = 1 + (parse_retry_delay_secs(error_message) or 5.0)
            if retry_attempts > 0:
                print(
                    f"{R}Failed to publish{X} {B}{name}{X}:\n{error_message}\n\nRemaining retry attempts: {retry_attempts}.\nRetrying in {retry_delay} seconds."
                )
                retry_attempts -= 1
                time.sleep(retry_delay + 1)
            else:
                print(
                    f"{R}Failed to publish{X} {B}{name}{X}:\n{error_message}\n\nNo remaining retry attempts; aborting publish"
                )
                raise


def publish_unpublished_crates_in_parallel(
    all_crates: dict[str, Crate], version: str, token: str, dry_run: bool
) -> None:
    # filter all_crates for any that are already published
    print("Collecting unpublished crates…")
    unpublished_crates: dict[str, Crate] = {}
    for name, crate in all_crates.items():
        if is_already_published(version, crate):
            print(f"{G}Already published{X} {B}{name}{X}@{B}{version}{X}")
        else:
            unpublished_crates[name] = crate

    # collect dependency graph (adjacency list of `crate -> dependencies`)
    print("Building dependency graph…")
    dependency_graph: dict[str, list[str]] = {}
    for name, crate in unpublished_crates.items():
        dependencies = []
        for dependency in crate_deps(crate.manifest):
            # NOTE: we _could_ theoretically ignore dev-dependencies here, as per the cargo book:
            # https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
            #
            # However, we historically had a `dev-dependency` cycle which caused endless headaches. So we're now
            # disallowing _any_ cycles.

            if dependency.name in unpublished_crates:
                dependencies.append(dependency.name)
        dependency_graph[name] = dependencies

    if len(unpublished_crates) == 0:
        # Building the DAG with an empty set of unpublished crates fails, so we exit early if no work needs to be done.
        print("All crates have already been published.")
        return

    # walk the dependency graph in parallel and publish each crate
    print(f"Publishing {len(unpublished_crates)} crates…")
    env = {**os.environ.copy(), "RERUN_IS_PUBLISHING_CRATES": "yes"}

    # The max token parameter attempts to model `crates.io` rate limiting. In dry run mode, we don't want to wait so
    # we seed the rate limiter with plenty of tokens.
    if dry_run:
        max_token = 1e3
    else:
        max_token = 30

    DAG(dependency_graph).walk_parallel(
        lambda name: publish_crate(unpublished_crates[name], token, version, env, dry_run),
        # 30 tokens per minute (burst limit in crates.io)
        rate_limiter=RateLimiter(max_tokens=max_token, refill_interval_sec=60),
        # publishing already uses all cores, don't start too many publishes at once
        num_workers=min(MAX_PUBLISH_WORKERS, cpu_count()),
    )


def check_dependency_tree() -> None:
    root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text(encoding="utf-8"))

    print("Collecting publishable crates…")
    workspace_crates = get_workspace_crates(root)

    ctx = Context()
    crates = get_sorted_publishable_crates(ctx, workspace_crates)

    dependency_graph: dict[str, list[str]] = {}
    for name, crate in crates.items():
        print(name)
        dependencies = []
        for dependency in crate_deps(crate.manifest):
            if dependency.name in crates:
                dependencies.append(dependency.name)
        dependency_graph[name] = dependencies

    try:
        # This runs a dependency graph sanitization and raises an exception on fail
        _dag = DAG(dependency_graph)
    except Exception as e:
        from pprint import pprint

        print("Full dependency graph:")
        pprint(dependency_graph)

        raise e


def publish(dry_run: bool, token: str) -> None:
    ctx = Context()

    root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text(encoding="utf-8"))
    version: str = root["workspace"]["package"]["version"]

    print("Collecting publishable crates…")
    workspace_crates = get_workspace_crates(root)
    crates = get_sorted_publishable_crates(ctx, workspace_crates)

    for name in crates.keys():
        ctx.publish(name, version)
    ctx.finish()

    publish_unpublished_crates_in_parallel(crates, version, token, dry_run)


def get_latest_published_version(crate_name: str, skip_prerelease: bool = False) -> str | None:
    resp = requests.get(
        f"https://crates.io/api/v1/crates/{crate_name}",
        headers={"user-agent": "rerun-publishing-script (rerun.io)"},
    )
    body = resp.json()

    if not resp.ok:
        detail = body["errors"][0]["detail"]
        if detail == "Not Found":
            # First time we're publishing this crate
            return None
        else:
            raise Exception(f"failed to get crate {crate_name}: {detail}")

    if "versions" not in body:
        return None

    # response orders versions by semver
    versions = body["versions"]

    if skip_prerelease:
        for version in versions:
            # no prerelease metadata
            if "-" not in version["num"]:
                return str(version["num"])
        raise RuntimeError(f"no non-prerelease versions found for crate {crate_name}")
    else:
        return str(versions[0]["num"])


class Target(Enum):
    Git = "git"
    CratesIo = "cratesio"

    def __str__(self) -> str:
        return self.value


def get_release_version_from_git_branch() -> str:
    return git.Repo().active_branch.name.removeprefix("prepare-release-")


def get_version(target: Target | None, skip_prerelease: bool = False) -> VersionInfo:
    if target is Target.Git:
        branch_name = get_release_version_from_git_branch()
        try:
            current_version = VersionInfo.parse(branch_name)  # ensures that it is a valid version
        except ValueError:
            print(f"the current branch `{branch_name}` does not specify a valid version.")
            print("this script expects the format `prepare-release-x.y.z-meta.N`")
            sys.exit(1)
    elif target is Target.CratesIo:
        latest_published_version = get_latest_published_version("rerun", skip_prerelease)
        if not latest_published_version:
            raise Exception("Failed to get latest published version for `rerun` crate")
        current_version = VersionInfo.parse(latest_published_version)
    else:
        root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text(encoding="utf-8"))
        current_version = VersionInfo.parse(root["workspace"]["package"]["version"])

    return current_version


def is_valid_version_string(version: str) -> bool:
    # remove metadata -> split into digits
    parts = version.split("-")[0].split(".")

    if len(parts) != 3:
        return False

    for part in parts:
        if not part.isdigit():
            return False

    return True


def check_git_branch_name() -> None:
    version = get_release_version_from_git_branch()

    if is_valid_version_string(version):
        print(f'"{version}" is a valid version string.')
    else:
        raise Exception(f'"{version}" is not a valid version string. See RELEASES.md for supported formats')


def check_publish_flags() -> None:
    root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text(encoding="utf-8"))
    crates = get_workspace_crates(root)

    def traverse(out: set[str], crates: dict[str, Crate], crate: Crate) -> None:
        if crate.manifest["package"]["name"] in out:
            return

        for dependency in crate_deps(crate.manifest):
            if dependency.name not in crates:
                continue

            dependency_manifest = crates[dependency.name].manifest
            should_publish_dependency = dependency_manifest["package"]["publish"]
            if not should_publish_dependency:
                out.add(dependency.name)

                # recurse into the publish=false dependency
                traverse(out, crates, crates[dependency.name])

    wrong_publish: set[str] = set()

    # traverse the crates, and for each one that is `publish=true`,
    # check that all of its workspace dependencies are `publish=true`.
    for crate in crates.values():
        should_publish = crate.manifest["package"]["publish"]
        if should_publish:
            traverse(wrong_publish, crates, crate)

    if len(wrong_publish) > 0:
        for name in wrong_publish:
            print(f"{name} needs to be changed to `publish=true`")
        sys.exit(1)
    else:
        print("All crates have the correct `publish` flag set.")
        return


def print_version(
    target: Target | None,
    finalize: bool = False,
    pre_id: bool = False,
    skip_prerelease: bool = False,
) -> None:
    current_version = get_version(target, skip_prerelease)

    if finalize:
        current_version = current_version.finalize_version()

    if pre_id:
        sys.stdout.write(str(current_version.prerelease.split(".", 1)[0]))  # type: ignore[union-attr]
        sys.stdout.flush()
    else:
        sys.stdout.write(str(current_version))
        sys.stdout.flush()


def main() -> None:
    colorama_init()
    parser = argparse.ArgumentParser(description="Generate a PR summary page")

    cmds_parser = parser.add_subparsers(title="cmds", dest="cmd")

    version_parser = cmds_parser.add_parser("version", help="Bump the crate versions")
    target_version_update_group = version_parser.add_mutually_exclusive_group()
    target_version_update_group.add_argument(
        "--bump",
        type=Bump,
        choices=list(Bump),
        help="Bump version according to semver",
    )
    target_version_update_group.add_argument("--exact", type=str, help="Update version to an exact value")
    dev_parser = version_parser.add_mutually_exclusive_group()
    dev_parser.add_argument("--dev", default=None, action="store_true", help="Set build metadata to `+dev`")
    dev_parser.add_argument(
        "--no-dev",
        dest="dev",
        action="store_false",
        help="Remove `+dev` from build metadata (if present)",
    )
    version_parser.add_argument("--dry-run", action="store_true", help="Display the execution plan")
    version_parser.add_argument(
        "--pre-id",
        type=str,
        default=DEFAULT_PRE_ID,
        choices=["alpha", "rc"],
        help="Set the pre-release prefix",
    )

    publish_parser = cmds_parser.add_parser("publish", help="Publish crates")
    publish_parser.add_argument("--token", type=str, help="crates.io token")
    publish_parser.add_argument("--dry-run", action="store_true", help="Display the execution plan")
    publish_parser.add_argument("--allow-dirty", action="store_true", help="Allow uncommitted changes")

    cmds_parser.add_parser("check-git-branch-name", help="Check if the git branch name uses the correct format")

    get_version_parser = cmds_parser.add_parser("get-version", help="Get the current crate version")
    get_version_parser.add_argument(
        "--finalize",
        action="store_true",
        help="Return version finalized if it is a pre-release",
    )
    get_version_parser.add_argument("--pre-id", action="store_true", help="Retrieve only the prerelease identifier")
    get_version_parser.add_argument(
        "--from",
        type=Target,
        choices=list(Target),
        help="Get version from git or crates.io",
        dest="target",
    )
    get_version_parser.add_argument(
        "--skip-prerelease",
        action="store_true",
        help="If target is cratesio, return the first non-prerelease version",
    )

    cmds_parser.add_parser(
        "check-publish-flags", help="Check if any publish=true crates depend on publish=false crates."
    )

    cmds_parser.add_parser("check-dependency-tree", help="Check that our dependency tree doesn't have any cycles.")

    args = parser.parse_args()

    if args.cmd == "check-git-branch-name":
        check_git_branch_name()
    if args.cmd == "check-publish-flags":
        check_publish_flags()
    if args.cmd == "check-dependency-tree":
        check_dependency_tree()
    if args.cmd == "get-version":
        print_version(args.target, args.finalize, args.pre_id, args.skip_prerelease)
    if args.cmd == "version":
        if args.dev and args.pre_id != "alpha":
            parser.error("`--pre-id` must be set to `alpha` when `--dev` is set")

        if args.bump is None and args.exact is None and args.dev is None:
            parser.error("one of `--bump`, `--exact`, `--dev` is required")

        if args.bump:
            bump_version(args.dry_run, args.bump, args.pre_id, args.dev)
        else:
            bump_version(args.dry_run, args.exact, args.pre_id, args.dev)
    if args.cmd == "publish":
        if not args.dry_run and not args.token:
            parser.error("`--token` is required when `--dry-run` is not set")
        publish(args.dry_run, args.token)


if __name__ == "__main__":
    main()
