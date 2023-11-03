#!/usr/bin/env node

import { $, script_dir, path, packages } from "./common.mjs";

const root_dir = path.resolve(script_dir, "..");

for (const pkg of packages) {
  const cwd = path.join(root_dir, pkg.path);
  $(`npm install`, { cwd });
}

