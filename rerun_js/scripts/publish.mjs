#!/usr/bin/env node
// @ts-check

import {
  $,
  fs,
  script_dir,
  path,
  fail,
  inferTag,
  isPublished,
} from "./common.mjs";

const root_dir = path.resolve(script_dir, "..");

if (!process.env.NODE_AUTH_TOKEN) {
  fail(
    `"NODE_AUTH_TOKEN" env is not set. https://docs.npmjs.com/creating-and-viewing-access-tokens`,
  );
}

/** @type {{ workspaces: string[] }} */
const root_package_json = JSON.parse(
  fs.readFileSync(path.join(root_dir, "package.json"), "utf-8"),
);

const all_packages = await Promise.all(
  root_package_json.workspaces.map(async (pkg) => {
    const dir = path.join(root_dir, pkg);
    const { name, version } = JSON.parse(
      fs.readFileSync(path.join(dir, "package.json"), "utf-8"),
    );
    const published = await isPublished(name, version);

    return { dir, name, version, published };
  }),
);

const unpublished = all_packages.filter((pkg) => {
  if (pkg.published) {
    console.log(`${pkg.name}@${pkg.version} is already published`);
    return false;
  }
  return true;
});

if (unpublished.length === 0) {
  console.log("nothing to publish");
  process.exit(0);
}

$(`yarn install`, { cwd: root_dir });

for (const pkg of unpublished) {
  console.log(`building ${pkg.name}@${pkg.version}`);
  $(`npm run build`, { cwd: pkg.dir });
}

for (const pkg of unpublished) {
  console.log(`publishing ${pkg.name}@${pkg.version}`);
  const tag = inferTag(pkg.version);
  $(`npm publish --tag ${tag}`, { cwd: pkg.dir });
}
