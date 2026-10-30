import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import { test } from "node:test";

// Run after `npm run build:js`: this checks the bundled output, not `index.ts`.
const bundle = fs.readFileSync(new URL("./index.js", import.meta.url), "utf-8");

const relative_imports = [
  ...bundle.matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*)["'](\.\.?\/[^"']*)["']/g),
].map((match) => match[1]);

const packed_files = new Set(
  JSON.parse(
    execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], {
      cwd: new URL(".", import.meta.url),
      encoding: "utf-8",
    }),
  )[0].files.map((file) => file.path),
);

test("the bundle has relative imports to check", () => {
  assert.notEqual(relative_imports.length, 0);
});

// Browsers resolve module specifiers literally, so an import that only a bundler can resolve
// breaks anyone serving the package as plain static files.
test("relative imports name a .js file that is published with the package", () => {
  for (const specifier of relative_imports) {
    assert.match(
      specifier,
      /\.js$/,
      `import of ${specifier} has no .js extension`,
    );
    const path = new URL(specifier, "file:///").pathname.slice(1);
    assert.ok(packed_files.has(path), `${path} is not in the npm package`);
  }
});

test("the wasm that re_viewer.js loads is published with the package", () => {
  assert.ok(packed_files.has("re_viewer_bg.wasm"));
});
