// @ts-check

import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs";

export const $ = /** @type {typeof execSync} */ (
  (cmd, opts) => {
    console.log(`${opts.cwd ?? ""}> ${cmd}`);
    return execSync(cmd, { stdio: "inherit", ...opts });
  }
);
export const argv = process.argv.slice(2);
export const script_dir = path.dirname(fileURLToPath(import.meta.url));

/**
 * Logs `message` and exits with code `1`.
 *
 * @type {(message: string) => never}
 */
export function fail(message) {
  console.error(message);
  process.exit(1);
}

/**
 * Checks that `version` is valid according to semver.
 *
 * @type {(version: string) => boolean}
 */
export function isSemver(version) {
  // https://semver.org/#is-there-a-suggested-regular-expression-regex-to-check-a-semver-string
  const RE =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;
  return RE.test(version);
}

/**
 * Strip the `+BUILD` from the version
 *
 * @type {(version: string) => string}
 */
export function stripSemverBuildMetadata(version) {
  if (!isSemver(version)) throw new Error(`${version} is not semver`);
  const idx = version.indexOf("+");
  if (idx === -1) {
    return version;
  } else {
    return version.slice(0, idx);
  }
}

/**
 * Returns the appropriate NPM package tag for `version`.
 *
 * @type {(version: string) => "alpha" | "rc" | "latest"}
 */
export function inferTag(version) {
  /** @type {"alpha" | "rc" | "latest"} */
  let tag = "latest";

  const [, prerelease] = version.split("-");
  if (prerelease) {
    const [kind, n] = prerelease.split(".");
    switch (kind) {
      case "alpha":
        tag = "alpha";
      case "rc":
        tag = "rc";
    }
  }

  return tag;
}

/**
 * Returns `true` if `package@version` is already published.
 *
 * @type {(packageName: string, version: string) => Promise<boolean>}
 */
export async function isPublished(packageName, version) {
  const response = await fetch(
    `https://registry.npmjs.org/${packageName}/${version}`,
  );
  return response.status === 200;
}

export { fs, path };
