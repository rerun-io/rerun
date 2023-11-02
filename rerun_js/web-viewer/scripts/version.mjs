#!/usr/bin/env node

import fs from "node:fs";
import {
  $,
  script_dir,
  path,
  argv,
  packages,
  fail,
  isSemver,
  stripSemverBuildMetadata,
} from "./common.mjs";

if (argv.length != 1) {
  fail("expected one positional argument: version");
}

const version = argv[0];
const root_dir = path.resolve(script_dir, "..");

if (!isSemver(version)) {
  fail(`${version} is not valid according to semver`);
}

for (const pkg of packages) {
  const package_json_path = path.join(root_dir, pkg.path, "package.json");
  const package_json = JSON.parse(fs.readFileSync(package_json_path));

  // update package version
  package_json.version = version;

  // update dependency versions
  if ("dependencies" in package_json) {
    for (const dependency of Object.keys(package_json.dependencies)) {
      if (dependency.startsWith("@rerun-io/web-viewer")) {
        package_json.dependencies[dependency] = stripSemverBuildMetadata(version);
      }
    }
  }

  fs.writeFileSync(package_json_path, JSON.stringify(package_json, null, 2));
}

