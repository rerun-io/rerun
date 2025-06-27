#!/usr/bin/env node
// @ts-check

import {
  fs,
  script_dir,
  path,
  argv,
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

const root_package_json = JSON.parse(
  fs.readFileSync(path.join(root_dir, "package.json"), "utf-8"),
);
for (const pkg_path of root_package_json.workspaces) {
  const package_json_path = path.join(root_dir, pkg_path, "package.json");
  const readme_path = path.join(root_dir, pkg_path, "README.md");
  const index_ts_path = path.join(root_dir, pkg_path, "index.ts");

  let package_json = JSON.parse(fs.readFileSync(package_json_path, "utf-8"));
  let readme = fs.readFileSync(readme_path, "utf-8");
  let index_ts = fs.readFileSync(index_ts_path, "utf-8");

  // update package version
  package_json.version = version;

  // update dependency versions
  if ("dependencies" in package_json) {
    for (const dependency of Object.keys(package_json.dependencies)) {
      if (dependency.startsWith("@rerun-io/")) {
        package_json.dependencies[dependency] =
          stripSemverBuildMetadata(version);
      }
    }
  }

  // we use `+dev` as a marker for "this version is unreleased",
  // which also means this link won't pass the link checker, so
  // skip the RRD link version bump here in that case.
  // this will be bumped only at the start of the release process,
  // when the `+dev` is removed. at that point the new version will
  // be uploaded, so the links will work, and this is safe to bump.
  if (!version.includes("+dev")) {
    readme = readme.replace(
      /<https:\/\/app\.rerun\.io\/.*\/examples\/dna\.rrd>/,
      `<https://app.rerun.io/version/${version}/examples/dna.rrd>`,
    );
    index_ts = index_ts.replace(
      /https:\/\/app\.rerun\.io\/.*\/examples\/dna\.rrd/,
      `https://app.rerun.io/version/${version}/examples/dna.rrd`,
    )
  }

  fs.writeFileSync(package_json_path, JSON.stringify(package_json, null, 2));
  fs.writeFileSync(readme_path, readme);
}
