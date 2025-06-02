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
  const watcher = await subscribe(
    path.join(process.cwd(), "src/js"),
    async () => {
      await build();
    },
  );

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
      // Minification doesn't help much with size, most of it is the embedded wasm binary.
      // What it _does_ do is cause most editors to be unable to open the file at all,
      // because it ends up being a single 30 MB-long line.
      //
      // minify: true,
      keepNames: true,
      outdir: "src/rerun_notebook/static",
    });
    log(`Built widget in ${Date.now() - start}ms`);
  } catch (e) {
    throw new Error(`Failed to rebuild widget:\n${e.toString()}`);
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
