// Script responsible for building the wasm and transforming the JS bindings for the web viewer.

import * as child_process from "node:child_process";
import { fileURLToPath } from "node:url";
import * as path from "node:path";
import * as fs from "node:fs";
import * as util from "node:util";

const __filename = path.resolve(fileURLToPath(import.meta.url));
const __dirname = path.dirname(__filename);

const exec = (/** @type {string} */ cmd) => {
  console.log(cmd);
  child_process.execSync(cmd, { cwd: __dirname, stdio: "inherit" });
};

function buildWebViewer(/** @type {"debug" | "release"} */ mode) {
  switch (mode) {
    case "debug": {
      return exec(
        "cargo run -p re_dev_tools -- build-web-viewer --debug --target no-modules-base -o rerun_js/web-viewer",
      );
    }
    case "release": {
      return exec(
        "cargo run -p re_dev_tools -- build-web-viewer --release -g --target no-modules-base -o rerun_js/web-viewer",
      );
    }
    default:
      throw new Error(`Unknown mode: ${mode}`);
  }
}

async function re_viewer_js(/** @type {"debug" | "release"} */ mode) {
  let code = fs.readFileSync(path.join(__dirname, "re_viewer.js"), "utf-8");
  const valid = await isHashValid(mode, "re_viewer.js", code);

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

  return valid;
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

async function hash(data) {
  const buffer = await crypto.subtle.digest("sha-256", data);
  return Array.from(new Uint8Array(buffer))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

async function isHashValid(
  /** @type {"debug" | "release"} */ mode,
  /** @type {string} */ id,
  /** @type {string} */ data,
) {
  const storedHash = hashes?.[mode]?.[id];
  const computedHash = await hash(new TextEncoder().encode(data));

  if (options.updateHashes) {
    hashes[mode] ??= {};
    hashes[mode][id] = computedHash;
    return true;
  }

  return storedHash === computedHash;
}

async function run(/** @type {"debug" | "release"} */ mode) {
  buildWebViewer(mode);
  const valid = await re_viewer_js(mode);
  re_viewer_d_ts(mode);

  if (options.checkHashes && !valid) {
    console.error(`
==============================================================
Output of "re_viewer.js" has changed.

The the output of \`wasm-bindgen\` is unstable, but we depend on it
for the web viewer. In order to make sure this didn't break anything,
you should:

1) Run \`pixi run js-build-base\`, and
2) Inspect the generated \`rerun_js/web-viewer/re_viewer.js\` file.

Most of the time, the hashes change because:

- Some field offsets changed,
- You added/removed/changed a closure passed into JS,
- You added/removed/changed a new exported symbol

Ideally, you would diff \`re_viewer.js\` against the same file generated
on the \`main\` branch to make sure that this is the case. You can disable
the hash checks by running:

  pixi run node rerun_js/web-viewer/build-wasm.mjs --mode release --no-check

Run \`pixi run js-update-hashes\` once you're sure that there are no issues.
==============================================================
      `);
    process.exit(1);
  }
}

const args = util.parseArgs({
  options: {
    mode: {
      type: "string",
    },
    "update-hashes": {
      type: "boolean",
    },
    "no-check": {
      type: "boolean",
    },
  },
});

let options = {
  updateHashes: !!args.values["update-hashes"],
  checkHashes: !args.values["no-check"],
};
let hashes;
try {
  hashes = JSON.parse(
    fs.readFileSync(path.join(__dirname, "hashes.json"), "utf-8"),
  );
} catch (e) {
  hashes = {};
}

try {
  if (options.updateHashes) {
    await run("release");
    await run("debug");
    fs.writeFileSync(
      path.join(__dirname, "hashes.json"),
      JSON.stringify(hashes),
    );
  } else {
    if (!args.values.mode) {
      throw new Error("Missing required argument: mode");
    }

    await run(args.values.mode);
  }
} catch (e) {
  console.error(e.message);
}
