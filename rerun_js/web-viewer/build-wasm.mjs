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

function buildWasm(mode) {
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

child_process.execSync(
  "cargo run -p re_dev_tools -- build-web-viewer --debug --target no-modules-base -o rerun_js/web-viewer",
  { cwd: __dirname, stdio: "inherit" },
);

function preprocessJs() {
  let code = fs.readFileSync(path.join(__dirname, "re_viewer.js"), "utf-8");

  const start = `let wasm_bindgen;
(function() {`;
  const end = `wasm_bindgen = Object.assign(__wbg_init, { initSync }, __exports);

})();`;
  code = code.replace(start, "").replace(end, "");

  code = `
export default function() {
${code}
return Object.assign(__wbg_init, { initSync }, __exports);
}
`;

  fs.writeFileSync(path.join(__dirname, "re_viewer.js"), code);
}

function preprocessDts() {
  let code = fs.readFileSync(path.join(__dirname, "re_viewer.d.ts"), "utf-8");

  code = `
${code}
export type WebHandle = wasm_bindgen.WebHandle;
export default function(): wasm_bindgen;
`;

  fs.writeFileSync(path.join(__dirname, "re_viewer.d.ts"), code);
}

const args = util.parseArgs({
  options: {
    mode: {
      type: "string",
    },
  },
});

if (!args.values.mode) {
  throw new Error("Missing required argument: mode");
}

buildWasm(args.values.mode);
preprocessJs();
preprocessDts();
