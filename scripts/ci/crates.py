#!/usr/bin/env python3

"""
Versioning and packaging.

Install dependencies:
    python3 -m pip install -r scripts/ci/requirements.txt

Use the script:
    python3 scripts/ci/crates.py --help

    # Update crate versions to the next prerelease version,
    # e.g. `0.8.0` -> `0.8.0-alpha.0`, `0.8.0-alpha.0` -> `0.8.0-alpha.1`
    python3 scripts/ci/crates.py version prerelase --dry-run

    # Publish all crates in topological order
    python3 scripts/ci/publish.py --token <CRATES_IO_TOKEN>
"""
from __future__ import annotations

import argparse
import os.path
import subprocess
from enum import Enum
from glob import glob
from pathlib import Path
from typing import Any, Dict, Generator, List, Tuple

import tomlkit
from colorama import Fore
from colorama import init as colorama_init
from semver import VersionInfo


def cargo(args: str, cwd: str | Path | None = None) -> None:
    subprocess.check_output(["cargo"] + args.split(), cwd=cwd)


class Crate:
    def __init__(self, manifest: Dict[str, Any], path: Path):
        self.manifest = manifest
        self.path = path


def get_workspace_crates(root: Dict[str, Any]) -> Dict[str, Crate]:
    """
    Returns a dictionary of workspace crates.

    The crates are in the same order as they appear in the root `Cargo.toml`
    under `workspace.members`.
    """

    crates: Dict[str, Crate] = {}
    for pattern in root["workspace"]["members"]:
        for crate in [member for member in glob(pattern) if os.path.isdir(member)]:
            crate_path = Path(crate)
            manifest_text = (crate_path / "Cargo.toml").read_text()
            manifest: Dict[str, Any] = tomlkit.parse(manifest_text)
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


def crate_deps(member: Dict[str, Dict[str, Any]]) -> Generator[Tuple[str, DependencyKind], None, None]:
    if "dependencies" in member:
        for v in member["dependencies"].keys():
            yield (v, DependencyKind.DIRECT)
    if "dev-dependencies" in member:
        for v in member["dev-dependencies"].keys():
            yield (v, DependencyKind.DEV)
    if "build-dependencies" in member:
        for v in member["build-dependencies"].keys():
            yield (v, DependencyKind.BUILD)


def get_sorted_publishable_crates(ctx: Context, crates: Dict[str, Crate]) -> Dict[str, Crate]:
    """
    Returns crates topologically sorted in publishing order.

    This also filters any crates which have `publish` set to `false`.
    """

    def helper(
        ctx: Context,
        crates: Dict[str, Crate],
        name: str,
        output: Dict[str, Crate],
        visited: Dict[str, bool],
    ) -> None:
        crate = crates[name]
        for dependency, _ in crate_deps(crate.manifest):
            if dependency not in crates:
                continue
            helper(ctx, crates, dependency, output, visited)
        # Insert only after all dependencies have been traversed
        if name not in visited:
            visited[name] = True
            if "publish" not in crate.manifest["package"]:
                ctx.error(
                    f"Crate {Fore.BLUE}{name}{Fore.RESET} does not have {Fore.BLUE}package.publish{Fore.RESET} set."
                )
            elif crate.manifest["package"]["publish"]:
                output[name] = crate

    visited: Dict[str, bool] = {}
    output: Dict[str, Crate] = {}
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


bump_fn = {
    Bump.MAJOR: VersionInfo.bump_major,
    Bump.MINOR: VersionInfo.bump_minor,
    Bump.PATCH: VersionInfo.bump_patch,
    Bump.PRERELEASE: VersionInfo.bump_prerelease,
    Bump.FINALIZE: VersionInfo.finalize_version,
}


def is_pinned(version: str) -> bool:
    return version.startswith("=")


class Context:
    ops: List[str] = []
    errors: List[str] = []

    def bump(self, path: str, prev: str, new: str) -> None:
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
            if dry_run:
                print("The following operations will be performed:")
            for op in self.ops:
                print(op)


def bump_package_version(
    ctx: Context,
    crate: str,
    new_version: str,
    manifest: Dict[str, Any],
) -> None:
    if "package" in manifest and "version" in manifest["package"]:
        version = manifest["package"]["version"]
        if "workspace" not in version or not version["workspace"]:
            ctx.bump(crate, version, new_version)
            manifest["package"]["version"] = new_version


def bump_dependency_versions(
    ctx: Context,
    crate: str,
    new_version: str,
    manifest: Dict[str, Any],
    crates: Dict[str, Crate],
) -> None:
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
            dependency_version = info["version"]
            pin_prefix = "=" if is_pinned(dependency_version) else ""
            ctx.bump(
                f"{crate}.{dependency}",
                dependency_version,
                pin_prefix + new_version,
            )
            info["version"] = pin_prefix + new_version


def version(dry_run: bool, bump: Bump) -> None:
    ctx = Context()

    root: Dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text())
    crates = get_workspace_crates(root)
    current_version = VersionInfo.parse(root["workspace"]["package"]["version"])
    new_version = str(bump_fn[bump](current_version))

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
        cargo("update -w")


def publish(dry_run: bool, token: str) -> None:
    ctx = Context()

    root: Dict[str, Any] = tomlkit.parse(Path("Cargo.toml").read_text())
    version = root["workspace"]["package"]["version"]
    crates = get_sorted_publishable_crates(ctx, get_workspace_crates(root))

    for name in crates.keys():
        ctx.publish(name, version)
    ctx.finish(dry_run)

    if not dry_run:
        for crate in crates.values():
            cargo("publish --dry-run", cwd=crate.path)
            cargo(f"publish --token {token}", cwd=crate.path)


def main() -> None:
    colorama_init()
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    cmds_parser = parser.add_subparsers(title="cmds", dest="cmd")
    version_parser = cmds_parser.add_parser("version", help="Bump the crate versions")
    version_parser.add_argument("bump", type=Bump, choices=list(Bump))
    version_parser.add_argument("--dry-run", action="store_true", help="Display the execution plan")
    publish_parser = cmds_parser.add_parser("publish", help="Publish crates")
    publish_parser.add_argument("--token", type=str, help="crates.io token")
    publish_parser.add_argument("--dry-run", action="store_true", help="Display the execution plan")
    publish_parser.add_argument("--allow-dirty", action="store_true", help="Allow uncommitted changes")
    args = parser.parse_args()

    if args.cmd == "version":
        version(args.dry_run, args.bump)
    if args.cmd == "publish":
        if not args.dry_run and not args.token:
            parser.error("`--token` is required when `--dry-run` is not set")
        publish(args.dry_run, args.token)


if __name__ == "__main__":
    main()
