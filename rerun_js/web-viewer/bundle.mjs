// @ts-check

// Script responsible for taking the generated Wasm/JS, and transpiled TS
// and producing a single file with everything inlined.

import { fileURLToPath } from "node:url";
import * as path from "node:path";
import * as fs from "node:fs";
import * as zlib from "node:zlib";
import * as util from "node:util";

const __filename = path.resolve(fileURLToPath(import.meta.url));
const __dirname = path.dirname(__filename);

const wasm = zlib.gzipSync(fs.readFileSync(path.join(__dirname, "re_viewer_bg.wasm")));
const js = fs.readFileSync(path.join(__dirname, "re_viewer.js"), "utf-8");
const index = fs.readFileSync(path.join(__dirname, "index.js"), "utf-8");

const INLINE_MARKER = "/*<INLINE-MARKER>*/";

/** @param {Buffer} buffer */
function buffer_to_data_url(buffer) {
  return `data:application/octet-stream;gzip;base64,${buffer.toString("base64")}`;
}

async function compressed_data_url_to_buffer(dataUrl) {
    const response = await fetch(dataUrl);
    const blob = await response.blob();

    let ds = new DecompressionStream("gzip");
    let decompressedStream = blob.stream().pipeThrough(ds);

    return await new Response(decompressedStream).arrayBuffer();
}

const inlined_js = js.replace("export default function", "return function");

const inlined_code = `
async function fetch_viewer_js() {
  ${inlined_js}
}

async function fetch_viewer_wasm() {
  ${compressed_data_url_to_buffer.toString()}
  const dataUrl = ${JSON.stringify(buffer_to_data_url(wasm))};
  const buffer = await compressed_data_url_to_buffer(dataUrl);
  return new Response(buffer, { "headers": { "Content-Type": "application/wasm" } });
}
`;

// replace INLINE_MARKER, inclusive
const inline_start = index.indexOf(INLINE_MARKER);
if (inline_start === -1) {
  throw new Error("no inline marker in source file");
}
let inline_end = index.indexOf(INLINE_MARKER, inline_start + 1);
if (inline_end === -1) {
  throw new Error("no inline marker in source file");
}
inline_end += INLINE_MARKER.length;

const bundle =
  index.substring(0, inline_start) + inlined_code + index.substring(inline_end);

fs.writeFileSync(path.join(__dirname, "inlined.js"), bundle);
fs.copyFileSync(
  path.join(__dirname, "index.d.ts"),
  path.join(__dirname, "inlined.d.ts"),
);
