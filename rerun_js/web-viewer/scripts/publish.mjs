#!/usr/bin/env node

import { $, script_dir, path, argv, packages, isPublished } from "./common.mjs";
import * as fs from "node:fs/promises";

/**
 * @typedef {{
 *   name: string,
 *   version: string,
 * }} PackageJson
 */

async function main() {
  const args = argv.join(" ");
  const root_dir = path.resolve(script_dir, "..");

  for (const pkg of packages) {
    const cwd = path.join(root_dir, pkg.path);

    /** @type {PackageJson} */
    const packageJson = await fs
      .readFile(path.join(cwd, "package.json"), "utf-8")
      .then((s) => JSON.parse(s));

    if (await isPublished(packageJson.name, packageJson.version)) {
      continue;
    }

    $(`npm install`, { cwd });
    $(`npm run build`, { cwd });
    $(`npm publish ${args}`, { cwd });
  }
}

main();

