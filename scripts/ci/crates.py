#!/usr/bin/env python3

"""
Versioning and packaging.

Install dependencies:
    python3 -m pip install -r scripts/ci/requirements.txt

Use the script:
    python3 scripts/ci/crates.py --help

    # Update crate versions to the next prerelease version,
    # e.g. `0.8.0` -> `0.8.0-alpha.0`, `0.8.0-alpha.0` -> `0.8.0-alpha.1`
    python3 scripts/ci/crates.py version --bump prerelase --dry-run

    # Update crate versions to an exact version
    python3 scripts/ci/crates.py version --exact 0.10.1 --dry-run

    # Publish all crates in topological order
    python3 scripts/ci/publish.py --token <CRATES_IO_TOKEN>
"""
from __future__ import annotations

import argparse
import os.path
import shutil
import subprocess
import sys
from enum import Enum
from glob import glob
from pathlib import Path
from time import sleep, time
from typing import Any, Generator

import git
import requests
import tomlkit
from colorama import Fore
from colorama import init as colorama_init
from semver import VersionInfo

CARGO_PATH = shutil.which("cargo") or "cargo"
DEFAULT_PRE_ID = "alpha"


def cargo(dry_run: bool, args: str, cwd: str | Path | None = None, env: dict[str, Any] = {}) -> Any:
    cmd = [CARGO_PATH] + args.split()
    # print(f"> {subprocess.list2cmdline(cmd)}")
    if not dry_run:
        subprocess.check_output(cmd, cwd=cwd, env=env)


class Crate:
    def __init__(self, manifest: dict[str, Any], path: Path):
        self.manifest = manifest
        self.path = path


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
            manifest_text = (crate_path / "Cargo.toml").read_text()
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


def crate_deps(member: dict[str, dict[str, Any]]) -> Generator[tuple[str, DependencyKind], None, None]:
    if "dependencies" in member:
        for v in member["dependencies"].keys():
            yield (v, DependencyKind.DIRECT)
    if "dev-dependencies" in member:
        for v in member["dev-dependencies"].keys():
            yield (v, DependencyKind.DEV)
    if "build-dependencies" in member:
        for v in member["build-dependencies"].keys():
            yield (v, DependencyKind.BUILD)


def get_sorted_publishable_crates(ctx: Context, crates: dict[str, Crate]) -> dict[str, Crate]:
    """
    Returns crates topologically sorted in publishing order.

    This also filters any crates which have `publish` set to `false`.
    """

    def helper(
        ctx: Context,
        crates: dict[str, Crate],
        name: str,
        output: dict[str, Crate],
        visited: dict[str, bool],
    ) -> None:
        crate = crates[name]
        for dependency, _ in crate_deps(crate.manifest):
            if dependency not in crates:
                continue
            helper(ctx, crates, dependency, output, visited)
        # Insert only after all dependencies have been traversed
        if name not in visited:
            visited[name] = True
            publish = crate.manifest["package"].get("publish")
            if publish is None:
                ctx.error(
                    f"Crate {Fore.BLUE}{name}{Fore.RESET} does not have {Fore.BLUE}package.publish{Fore.RESET} set."
                )
                return

            if publish:
                output[name] = crate

    visited: dict[str, bool] = {}
    output: dict[str, Crate] = {}
    for name in crates.keys():
        helper(ctx, crates, name, output, visited)
    return output


class Bump(Enum):
    MAJOR = "major"
    MINOR = "minor"
    PATCH = "patch"
    PRERELEASE = "prerelease"
    FINALIZE = "finalize"

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


def is_pinned(version: str) -> bool:
    return version.startswith("=")


class Context:
    ops: list[str] = []
    errors: list[str] = []

    def bump(self, path: str, prev: str, new: VersionInfo) -> None:
        # fmt: off
        op = " ".join([
            f"bump {Fore.BLUE}{path}{Fore.RESET}",
            f"from {Fore.GREEN}{prev}{Fore.RESET}",
            f"to {Fore.GREEN}{new}{Fore.RESET}",
        ])
        # fmt: on
        self.ops.append(op)

    def publish(self, crate: str, version: str) -> None:
        # fmt: off
        op = " ".join([
            f"publish {Fore.BLUE}{crate}{Fore.RESET}",
            f"version {Fore.GREEN}{version}{Fore.RESET}",
        ])
        # fmt: on
        self.ops.append(op)

    def plan(self, operation: str) -> None:
        self.ops.append(operation)

    def error(self, *e: str) -> None:
        self.errors.append("\n".join(e))

    def finish(self, dry_run: bool) -> None:
        if len(self.errors) > 0:
            print("Encountered some errors:")
            for error in self.errors:
                print(error)
            exit(1)
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
    for dependency, kind in crate_deps(manifest):
        if dependency not in crates:
            continue

        info = manifest[kind.manifest_key()][dependency]
        if isinstance(info, str):
            ctx.error(
                f"{crate}.{dependency} should be specified as:",
                f'  {dependency} = {{ version = "' + info + '" }',
            )
        elif "version" in info:
            pin_prefix = "=" if new_version.prerelease is not None else ""
            update_to = pin_prefix + str(new_version)
            ctx.bump(
                f"{crate}.{dependency}",
                info["version"],
                update_to,
            )
            info["version"] = update_to


def version(dry_run: bool, bump: Bump | str | None, pre_id: str, dev: bool) -> None:
    ctx = Context()

    root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text())
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

    ctx.finish(dry_run)

    # Save after bumping all versions
    if not dry_run:
        with Path("Cargo.toml").open("w") as f:
            tomlkit.dump(root, f)
        for name, crate in crates.items():
            with Path(f"{crate.path}/Cargo.toml").open("w") as f:
                tomlkit.dump(crate.manifest, f)
    cargo(dry_run, "update --workspace")


def is_already_uploaded(version: str, crate: Crate) -> bool:
    res = requests.get(
        f"https://crates.io/api/v1/crates/{crate}",
        headers={"user-agent": "rerun-publishing-script (rerun.io)"},
    ).json()

    # crate has not been uploaded yet
    if "versions" not in res:
        return False

    # crate has been uploaded, check every version against what we're uploading
    versions: list[str] = [version["num"] for version in res["versions"]]
    for uploaded_version in versions:
        if uploaded_version == version:
            return True
    return False


def publish_crate(dry_run: bool, crate: Crate, token: str, version: str, env: dict[str, Any]) -> None:
    package = crate.manifest["package"]
    name = package["name"]
    crate_version = crate.manifest["package"].get("version") or version
    if "workspace" in crate_version:
        crate_version = version

    if is_already_uploaded(crate_version, crate.manifest["package"]["name"]):
        print(f"{Fore.GREEN}Already published{Fore.RESET} {Fore.BLUE}{name}{Fore.RESET}")
    else:
        print(f"{Fore.GREEN}Publishing{Fore.RESET} {Fore.BLUE}{name}{Fore.RESET}…")
        try:
            cargo(dry_run, f"publish --quiet --token {token}", cwd=crate.path, env=env)
            print(f"{Fore.GREEN}Published{Fore.RESET} {Fore.BLUE}{name}{Fore.RESET}")
        except:
            print(f"Failed to publish {Fore.BLUE}{name}{Fore.RESET}")
            raise
    print()


def publish(dry_run: bool, token: str) -> None:
    ctx = Context()

    root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text())
    version = root["workspace"]["package"]["version"]
    print("Gather publishable crates…")
    crates = get_sorted_publishable_crates(ctx, get_workspace_crates(root))

    for name in crates.keys():
        ctx.publish(name, version)
    ctx.finish(dry_run)

    if not dry_run:
        env = {**os.environ.copy(), "RERUN_IS_PUBLISHING": "yes"}
        for crate in crates.values():
            start_s = time()
            publish_crate(dry_run, crate, token, version, env)
            elapsed_s = time() - start_s
            if elapsed_s < 1:
                sleep(1 - elapsed_s)


def get_version(finalize: bool, from_git: bool, pre_id: bool) -> None:
    if not from_git:
        root: dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text())
        current_version = VersionInfo.parse(root["workspace"]["package"]["version"])
    else:
        branch_name = git.Repo().active_branch.name.lstrip("release-")
        try:
            current_version = VersionInfo.parse(branch_name)  # ensures that it is a valid version
        except ValueError:
            print(f"the current branch `{branch_name}` does not specify a valid version.")
            print("this script expects the format `release-x.y.z-meta.N`")
            exit(1)

    if finalize:
        current_version = current_version.finalize_version()

    if pre_id:
        sys.stdout.write(str(current_version.prerelease.split(".", 1)[0]))
        sys.stdout.flush()
    else:
        sys.stdout.write(str(current_version))
        sys.stdout.flush()


def main() -> None:
    colorama_init()
    parser = argparse.ArgumentParser(description="Generate a PR summary page")

    cmds_parser = parser.add_subparsers(title="cmds", dest="cmd")

    version_parser = cmds_parser.add_parser("version", help="Bump the crate versions")
    target_version_parser = version_parser.add_mutually_exclusive_group()
    target_version_parser.add_argument("--bump", type=Bump, choices=list(Bump), help="Bump version according to semver")
    target_version_parser.add_argument("--exact", type=str, help="Update version to an exact value")
    dev_parser = version_parser.add_mutually_exclusive_group()
    dev_parser.add_argument("--dev", default=None, action="store_true", help="Set build metadata to `+dev`")
    dev_parser.add_argument(
        "--no-dev", dest="dev", action="store_false", help="Remove `+dev` from build metadata (if present)"
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

    get_version_parser = cmds_parser.add_parser("get-version", help="Get the current crate version")
    get_version_parser.add_argument(
        "--finalize", action="store_true", help="Return version finalized if it is a pre-release"
    )
    get_version_parser.add_argument("--from-git", action="store_true", help="Get version from branch name")
    get_version_parser.add_argument("--pre-id", action="store_true", help="Retrieve only the prerelease identifier")
    args = parser.parse_args()

    if args.cmd == "get-version":
        get_version(args.finalize, args.from_git, args.pre_id)
    if args.cmd == "version":
        if args.dev and args.pre_id != "alpha":
            parser.error("`--pre-id` must be set to `alpha` when `--dev` is set")

        if args.bump is None and args.exact is None and args.dev is None:
            parser.error("one of `--bump`, `--exact`, `--dev` is required")

        if args.bump:
            version(args.dry_run, args.bump, args.pre_id, args.dev)
        else:
            version(args.dry_run, args.exact, args.pre_id, args.dev)
    if args.cmd == "publish":
        if not args.dry_run and not args.token:
            parser.error("`--token` is required when `--dry-run` is not set")
        publish(args.dry_run, args.token)


if __name__ == "__main__":
    main()
