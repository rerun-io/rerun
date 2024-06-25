import esbuild from "esbuild";
import { wasmLoader } from "esbuild-plugin-wasm";
import { subscribe } from "@parcel/watcher";
import { parseArgs } from "node:util";
import path from "node:path";

// parse `--watch` file argument using node.js utils
const args = parseArgs({ options: { watch: { type: "boolean", short: "w" } } });

if (args.values.watch) {
  await watch();
} else {
  await build();
}

async function watch() {
  const watcher = await subscribe(path.join(process.cwd(), "src/js"), async () => {
    await build();
  });

  process.on("SIGINT", async () => {
    await watcher.unsubscribe();
    process.exit(0);
  });

  await build(); // initial build

  log("Watching for changes…");
}

async function build() {
  try {
    const start = Date.now();
    await esbuild.build({
      entryPoints: ["src/js/widget.ts"],
      bundle: true,
      format: "esm",
      minify: true,
      outdir: "src/rerun_notebook/static",
      plugins: [wasmLoader({ mode: "embedded" })],
    });
    log(`Built widget in ${Date.now() - start}ms`);
  } catch (e) {
    error(`Failed to rebuild widget:\n${e.toString()}`);
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

