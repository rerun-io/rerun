#!/usr/bin/env node

import { $, script_dir, path, argv, packages } from "./common.mjs";

const args = argv.join(" ");
const root_dir = path.resolve(script_dir, "..");

for (const pkg of packages) {
  const cwd = path.join(root_dir, pkg.path);
  $(`npm publish ${args}`, { cwd });
}

