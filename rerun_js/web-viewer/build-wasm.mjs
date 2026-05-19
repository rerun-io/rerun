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
      "--no-default-features",
      "--features map_view", // no `analytics`
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
  // https://github.com/wasm-bindgen/wasm-bindgen/issues/3130
  //
  // this is HIGHLY sensitive to the exact output of `wasm-bindgen`, so if
  // the output changes, this will need to be updated.

  const start = `let wasm_bindgen = (function(exports) {`;
  const end = `return Object.assign(__wbg_init, { initSync }, exports);
})({ __proto__: null });`;

  if (code.indexOf(start) === -1) {
    throw new Error("failed to run js build script: failed to patch re_viewer.js, could not find replace start marker");
  }
  if (code.indexOf(end) === -1) {
    throw new Error("failed to run js build script: failed to patch re_viewer.js, could not find replace end marker");
  }
  code = code.replace(start, "").replace(end, "");

  code = `
export default function() {
const exports = { __proto__: null };
${code}

function deinit() {
  __wbg_init.__wbindgen_wasm_module = null;
  wasmModule = null;
  wasm = null;
  cachedDataViewMemory0 = null;
  cachedFloat32ArrayMemory0 = null;
  cachedInt16ArrayMemory0 = null;
  cachedInt32ArrayMemory0 = null;
  cachedInt8ArrayMemory0 = null;
  cachedUint16ArrayMemory0 = null;
  cachedUint32ArrayMemory0 = null;
  cachedUint8ArrayMemory0 = null;
}

return Object.assign(__wbg_init, { initSync, deinit }, exports);
}
`;

  // Since we are nulling `wasm` we also have to patch the closure destructor code to let things be cleaned up fully.
  // Otherwise we end up with an exceptioon during closure destruction which prevents the references from all being
  // cleaned up properly.
  // TODO(jprochazk): Can we force these to run before we null `wasm` instead?
  // Patch CLOSURE_DTORS to guard against null `wasm` during deinit.
  // The FinalizationRegistry callback may fire after we've nulled `wasm`,
  // so we need to check that wasm is still alive before calling the destructor.
  const closure_dtors_original = `const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));`;

  const closure_dtors_patch = `const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(state => {
        if (wasm) wasm.__wbindgen_destroy_closure(state.a, state.b);
    });`;

  if (code.indexOf(closure_dtors_original) === -1) {
    throw new Error("failed to run js build script: failed to patch re_viewer.js, could not find CLOSURE_DTORS block");
  }

  code = code.replace(closure_dtors_original, closure_dtors_patch);

  // Patch makeMutClosure to guard against null `wasm` during deinit.
  // After deinit, pending async callbacks (requestAnimationFrame, setTimeout, etc.)
  // may still fire and try to invoke closures that call into the now-null `wasm`.
  // We guard both the closure invocation and the destructor call.
  const make_mut_closure_original = `function makeMutClosure(arg0, arg1, f) {
        const state = { a: arg0, b: arg1, cnt: 1 };
        const real = (...args) => {

            // First up with a closure we increment the internal reference
            // count. This ensures that the Rust closure environment won't
            // be deallocated while we're invoking it.
            state.cnt++;
            const a = state.a;
            state.a = 0;
            try {
                return f(a, state.b, ...args);
            } finally {
                state.a = a;
                real._wbg_cb_unref();
            }
        };
        real._wbg_cb_unref = () => {
            if (--state.cnt === 0) {
                wasm.__wbindgen_destroy_closure(state.a, state.b);
                state.a = 0;
                CLOSURE_DTORS.unregister(state);
            }
        };
        CLOSURE_DTORS.register(real, state, state);
        return real;
    }`;

  const make_mut_closure_patch = `function makeMutClosure(arg0, arg1, f) {
        const state = { a: arg0, b: arg1, cnt: 1 };
        const real = (...args) => {
            state.cnt++;
            const a = state.a;
            state.a = 0;
            try {
                if (!wasm) return;
                return f(a, state.b, ...args);
            } finally {
                state.a = a;
                real._wbg_cb_unref();
            }
        };
        real._wbg_cb_unref = () => {
            if (--state.cnt === 0) {
                if (wasm) wasm.__wbindgen_destroy_closure(state.a, state.b);
                state.a = 0;
                CLOSURE_DTORS.unregister(state);
            }
        };
        CLOSURE_DTORS.register(real, state, state);
        return real;
    }`;

  if (code.indexOf(make_mut_closure_original) === -1) {
    throw new Error("failed to run js build script: failed to patch re_viewer.js, could not find makeMutClosure block");
  }

  code = code.replace(make_mut_closure_original, make_mut_closure_patch);

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
  process.exit(1);
}
