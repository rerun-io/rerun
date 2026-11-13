// Script responsible for building the wasm and transforming the JS bindings for the web viewer.

import * as childProcess from "node:child_process";
import { fileURLToPath } from "node:url";
import * as path from "node:path";
import * as fs from "node:fs";
import * as util from "node:util";

const filename = path.resolve(fileURLToPath(import.meta.url));
const dirname = path.dirname(filename);

const exec = (cmd) => {
  console.log(cmd);
  childProcess.execSync(cmd, { cwd: dirname, stdio: "inherit" });
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
      "python ../../scripts/build_web_viewer.py",
      modeFlags,
      "--target no-modules-base",
      "--features default_web", // no `analytics`
      "-o rerun_js/web-viewer",
    ].join(" "),
  );
}

function reViewerJs() {
  let code = fs.readFileSync(path.join(dirname, "re_viewer.js"), "utf-8");

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
    throw new Error(
      "failed to run js build script: failed to patch re_viewer.js, could not find replace start marker",
    );
  }
  if (code.indexOf(end) === -1) {
    throw new Error(
      "failed to run js build script: failed to patch re_viewer.js, could not find replace end marker",
    );
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
  const closureDtorsOriginal = `const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));`;

  const closureDtorsPatch = `const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(state => {
        if (wasm) wasm.__wbindgen_destroy_closure(state.a, state.b);
    });`;

  if (code.indexOf(closureDtorsOriginal) === -1) {
    throw new Error(
      "failed to run js build script: failed to patch re_viewer.js, could not find CLOSURE_DTORS block",
    );
  }

  code = code.replace(closureDtorsOriginal, closureDtorsPatch);

  // Patch makeMutClosure to guard against null `wasm` during deinit.
  // After deinit, pending async callbacks (requestAnimationFrame, setTimeout, etc.)
  // may still fire and try to invoke closures that call into the now-null `wasm`.
  // We guard both the closure invocation and the destructor call.
  const makeMutClosureOriginal = `function makeMutClosure(arg0, arg1, f) {
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

  const makeMutClosurePatch = `function makeMutClosure(arg0, arg1, f) {
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

  if (code.indexOf(makeMutClosureOriginal) === -1) {
    throw new Error(
      "failed to run js build script: failed to patch re_viewer.js, could not find makeMutClosure block",
    );
  }

  code = code.replace(makeMutClosureOriginal, makeMutClosurePatch);

  fs.writeFileSync(path.join(dirname, "re_viewer.js"), code);
}

function reViewerDTs() {
  let code = fs.readFileSync(path.join(dirname, "re_viewer.d.ts"), "utf-8");

  // this transformation just re-exports WebHandle and adds a default export inside the `.d.ts` file

  code = `
${code}
export type WebHandle = wasm_bindgen.WebHandle;
export default function(): wasm_bindgen;
`;

  fs.writeFileSync(path.join(dirname, "re_viewer.d.ts"), code);
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
  reViewerJs();
  reViewerDTs();
}

try {
  main();
} catch (e) {
  console.error(e);
  process.exit(1);
}
