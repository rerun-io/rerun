#!/usr/bin/env node

import { $, script_dir, path, argv, packages } from "./common.mjs";

if (argv.length == 0) {
  console.error("missing arguments: expected one of patch, minor, major, or <exact version>");
  process.exit(1);
}

const args = argv.join(" ");
const root_dir = path.resolve(script_dir, "..");

for (const pkg of packages) {
  const cwd = path.join(root_dir, pkg);
  $(`npm version ${args}`, { cwd });
}

