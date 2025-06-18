import esbuild from "esbuild";
import fs from "node:fs";

async function main() {
  try {
    const start = Date.now();
    await esbuild.build({
      entryPoints: ["src/js/widget.ts"],
      bundle: true,
      format: "esm",
      // Minification doesn't help much with size, most of it is the embedded wasm binary.
      // What it _does_ do is cause most editors to be unable to open the file at all,
      // because it ends up being a single 30 MB-long line.
      //
      // minify: true,
      legalComments: "inline",
      keepNames: true,
      outdir: "src/rerun_notebook/static",
    });
    fs.copyFileSync(
      "node_modules/@rerun-io/web-viewer/re_viewer_bg.wasm",
      "src/rerun_notebook/static/re_viewer_bg.wasm",
    );
    log(`Built widget in ${Date.now() - start}ms`);
  } catch (e) {
    throw new Error(`Failed to build widget:\n${e.toString()}`);
  }
}

function now() {
  return new Date().toLocaleTimeString("en-US", { hour12: false });
}

/** @param {string} message */
function log(message) {
  return console.log(`[${now()}] ${message}`);
}

/** @param {string} message */
function error(message) {
  return console.error(`[${now()}] ${message}`);
}

await main();
