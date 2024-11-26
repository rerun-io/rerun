// Script responsible for building the wasm and transforming the JS bindings for the web viewer.

import * as child_process from "node:child_process";
import { fileURLToPath } from "node:url";
import * as path from "node:path";
import * as fs from "node:fs";
import * as util from "node:util";

const __filename = path.resolve(fileURLToPath(import.meta.url));
const __dirname = path.dirname(__filename);

const exec = (cmd) => {
  console.log(cmd);
  child_process.execSync(cmd, { cwd: __dirname, stdio: "inherit" });
};

function buildWebViewer(mode) {
  let modeFlags = "";
  switch (mode) {
    case "debug":
      modeFlags = "--debug";
      break;
    case "release":
      modeFlags = "--release -g";
      break;
    default:
      throw new Error(`Unknown mode: ${mode}`);
  }
  return exec(
    [
      "cargo run -p re_dev_tools -- build-web-viewer",
      modeFlags,
      "--target no-modules-base",
      "--features map_view,grpc",
      "-o rerun_js/web-viewer",
    ].join(" "),
  );
}

function re_viewer_js() {
  let code = fs.readFileSync(path.join(__dirname, "re_viewer.js"), "utf-8");

  // this transforms the module, wrapping it in a default-exported function.
  // calling the function produces a new "instance" of the module, because
  // all of the globals are scoped to the function, and become closure state
  // for any functions that reference them within the module.
  //
  // we do this so that we don't leak globals across web viewer instantiations:
  // https://github.com/rustwasm/wasm-bindgen/issues/3130
  //
  // this is HIGHLY sensitive to the exact output of `wasm-bindgen`, so if
  // the output changes, this will need to be updated.

  const start = `let wasm_bindgen;
(function() {`;
  const end = `wasm_bindgen = Object.assign(__wbg_init, { initSync }, __exports);

})();`;
  code = code.replace(start, "").replace(end, "");

  code = `
export default function() {
${code}

function deinit() {
  __wbg_init.__wbindgen_wasm_module = null;
  wasm = null;
  cachedFloat32ArrayMemory0 = null;
  cachedInt32ArrayMemory0 = null;
  cachedUint32ArrayMemory0 = null;
  cachedUint8ArrayMemory0 = null;
}

return Object.assign(__wbg_init, { initSync, deinit }, __exports);
}
`;

  // Since we are nulling `wasm` we also have to patch the closure destructor code to let things be cleaned up fully.
  // Otherwise we end up with an exceptioon during closure destruction which prevents the references from all being
  // cleaned up properly.
  // TODO(jprochazk): Can we force these to run before we null `wasm` instead?
  const closure_dtors = `const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => {
    wasm.__wbindgen_export_3.get(state.dtor)(state.a, state.b)`;

  const closure_dtors_patch = `const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => {
    wasm?.__wbindgen_export_3.get(state.dtor)(state.a, state.b)`;

  code = code.replace(closure_dtors, closure_dtors_patch);

  fs.writeFileSync(path.join(__dirname, "re_viewer.js"), code);
}

function re_viewer_d_ts() {
  let code = fs.readFileSync(path.join(__dirname, "re_viewer.d.ts"), "utf-8");

  // this transformation just re-exports WebHandle and adds a default export inside the `.d.ts` file

  code = `
${code}
export type WebHandle = wasm_bindgen.WebHandle;
export default function(): wasm_bindgen;
`;

  fs.writeFileSync(path.join(__dirname, "re_viewer.d.ts"), code);
}

function main() {
  const args = util.parseArgs({
    options: {
      mode: {
        type: "string",
      },
    },
  });
  const mode = args.values.mode;

  if (!mode) {
    throw new Error("Missing required argument: mode");
  }

  buildWebViewer(mode);
  re_viewer_js();
  re_viewer_d_ts();
}

try {
  main();
} catch (e) {
  console.error(e);
}
