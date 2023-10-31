#!/usr/bin/env node

import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

/** @type {typeof execSync} */
const $ = (...args) => execSync(...args);

const args = process.argv.slice(2).join(" ");
if (args.length == 0) {
  console.error("missing arguments: expected one of patch, minor, major, or <exact version>");
  process.exit(1);
}

const script_dir = path.dirname(fileURLToPath(import.meta.url));

$(`npm version ${args}`, { cwd: script_dir });
$(`npm version ${args}`, { cwd: path.join(script_dir, "react") });

