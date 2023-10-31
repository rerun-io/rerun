#!/usr/bin/env node

import { $, script_dir, path, argv } from "./common.mjs";

const args = argv.join(" ");
const root_dir = path.resolve(script_dir, "..");

$(`npm publish ${args}`, { cwd: root_dir, stdio: "inherit" });
$(`npm publish ${args}`, { cwd: path.join(root_dir, "react"), stdio: "inherit" });

