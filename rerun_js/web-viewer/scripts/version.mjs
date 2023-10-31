#!/usr/bin/env node

import { $, script_dir, path, argv } from "./common.mjs";

if (argv.length == 0) {
  console.error("missing arguments: expected one of patch, minor, major, or <exact version>");
  process.exit(1);
}

const args = argv.join(" ");
const root_dir = path.resolve(script_dir, "..");

$(`npm version ${args}`, { cwd: root_dir, stdio: "inherit" });
$(`npm version ${args}`, { cwd: path.join(root_dir, "react"), stdio: "inherit" });

