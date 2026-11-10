#!/usr/bin/env node
// @ts-check

import { $, fs, scriptDir, path, inferTag, isPublished } from "./common.mjs";

const rootDir = path.resolve(scriptDir, "..");

/** @type {{ workspaces: string[] }} */
const rootPackageJson = JSON.parse(fs.readFileSync(path.join(rootDir, "package.json"), "utf-8"));

const allPackages = await Promise.all(
  rootPackageJson.workspaces.map(async (pkg) => {
    const dir = path.join(rootDir, pkg);
    const { name, version } = JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf-8"));
    const published = await isPublished(name, version);

    return { dir, name, version, published };
  }),
);

const unpublished = allPackages.filter((pkg) => {
  if (pkg.published) {
    console.log(`${pkg.name}@${pkg.version} is already published`);
    return false;
  }
  return true;
});

$(`npm ci`, { cwd: rootDir });

for (const pkg of allPackages) {
  console.log(`building ${pkg.name}@${pkg.version}`);
  $(`npm run build`, { cwd: pkg.dir });
}

if (unpublished.length === 0) {
  console.log("nothing to publish");
} else {
  for (const pkg of unpublished) {
    console.log(`publishing ${pkg.name}@${pkg.version}`);
    const tag = inferTag(pkg.version);
    $(`npm publish --tag ${tag}`, { cwd: pkg.dir });
  }
}

const tarballs = [];
for (const pkg of allPackages) {
  $(`npm run pack`, { cwd: pkg.dir });
  const filename = `${pkg.name.split("/")[1]}.tar.gz`;
  tarballs.push(path.join(pkg.dir, filename));
}

console.log("constructing final package for GCS upload");
console.log(`files: ${tarballs.join(" ")}`);
const rerunJsPackageDir = "rerun_js_package";
fs.mkdirSync(rerunJsPackageDir);
for (const tarball of tarballs) {
  const dest = path.join(rerunJsPackageDir, path.basename(tarball));
  fs.copyFileSync(tarball, dest);
}
