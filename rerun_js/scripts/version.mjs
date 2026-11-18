#!/usr/bin/env node
// @ts-check

import { fs, scriptDir, path, argv, fail, isSemver, stripSemverBuildMetadata } from "./common.mjs";

if (argv.length != 1) {
  fail("expected one positional argument: version");
}

const version = argv[0];
const rootDir = path.resolve(scriptDir, "..");

if (!isSemver(version)) {
  fail(`${version} is not valid according to semver`);
}

const rootPackageJson = JSON.parse(fs.readFileSync(path.join(rootDir, "package.json"), "utf-8"));
for (const pkgPath of rootPackageJson.workspaces) {
  const packageJsonPath = path.join(rootDir, pkgPath, "package.json");
  const readmePath = path.join(rootDir, pkgPath, "README.md");
  const indexTsPath = path.join(rootDir, pkgPath, "index.ts");

  let packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf-8"));
  let readme = fs.readFileSync(readmePath, "utf-8");
  let indexTs = fs.existsSync(indexTsPath) ? fs.readFileSync(indexTsPath, "utf-8") : null;

  // update package version
  packageJson.version = version;

  // update dependency versions
  if ("dependencies" in packageJson) {
    for (const dependency of Object.keys(packageJson.dependencies)) {
      if (dependency.startsWith("@rerun-io/")) {
        packageJson.dependencies[dependency] = stripSemverBuildMetadata(version);
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
      /<https:\/\/app\.rerun\.io\/version\/.*\/examples\/dna\.rrd>/,
      `<https://app.rerun.io/version/${version}/examples/dna.rrd>`,
    );
    // pre-releases deploy their JS docs to `dev`, only final releases get a versioned directory.
    const jsDocsVersion = stripSemverBuildMetadata(version).includes("-") ? "dev" : version;
    readme = readme.replace(
      /https:\/\/ref\.rerun\.io\/docs\/js\/[^/]+\/web-viewer/g,
      `https://ref.rerun.io/docs/js/${jsDocsVersion}/web-viewer`,
    );
    if (indexTs) {
      indexTs = indexTs.replace(
        /https:\/\/app\.rerun\.io\/version\/.*\/examples\/dna\.rrd/,
        `https://app.rerun.io/version/${version}/examples/dna.rrd`,
      );
    }
  }

  fs.writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2));
  fs.writeFileSync(readmePath, readme);
  if (indexTs) {
    fs.writeFileSync(indexTsPath, indexTs);
  }
}
