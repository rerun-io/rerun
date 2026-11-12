// A page that starts a `WebViewer`, for the Playwright tests and for trying the API by hand.
//
// The tests reach the viewer through `window.viewer` and the package through `window.rerun`.
// By hand, pick a call, edit it if you like, and run it: the result shows up below.

import * as rerun from "@rerun-io/web-viewer";

declare global {
  interface Window {
    viewer: rerun.WebViewer;
    rerun: typeof rerun;
  }
}

/** A ready-made call for each part of the API, with `viewer` and `rerun` in scope. */
const SNIPPETS: Record<string, string> = {
  getState: "await viewer.getState()",
  setTime: 'await viewer.setTime({ sequence: 0, playState: "paused" })',
  play: 'await viewer.setTime({ playState: "playing" })',
  follow: 'await viewer.setTime({ playState: "following" })',
  getLocation: "await viewer.getLocation()",
  setLocation: "await viewer.setLocation((await viewer.getState()).recordings[0].location)",
  open: 'await viewer.open(new URL("/example.rrd", location.href).href)',
  close: 'await viewer.close("current")',
  getLogs: "await viewer.getLogs()",
  getRecordingSchema: "await viewer.getRecordingSchema()",
  getDatasetSchema: "await viewer.getDatasetSchema()",
  getDatasetAssetsSchema: "await viewer.getDatasetAssetsSchema()",
  getTableSchema: "await viewer.getTableSchema()",
  getBlueprint: 'await viewer.getBlueprint({ kind: "recording" })',
  setBlueprint:
    'await viewer.setBlueprint((await viewer.getBlueprint({ kind: "recording" })).blueprint, { kind: "recording" })',
  getTransformGraph: "await viewer.getTransformGraph()",
  takeScreenshot: "(await viewer.takeScreenshot({ maxSize: 256 })).byteLength",
  highlightRect: 'await viewer.highlightRect({ x: 20, y: 20, w: 200, h: 100 }, "Look here")',
  listCommands: "(await viewer.listCommands()).map((command) => command.id)",
  runCommand: "await viewer.runCommand((await viewer.listCommands())[0].id)",
  overridePanelState: 'viewer.overridePanelState("blueprint", "hidden")',
  togglePanelOverrides: "viewer.togglePanelOverrides()",
  saveRecording: "(await new Response(viewer.saveRecording()).arrayBuffer()).byteLength",
  ready: "viewer.ready",
};

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const snippets = $<HTMLSelectElement>("snippets");
const code = $<HTMLTextAreaElement>("code");
const output = $("output");

/** `value` as text, with `bigint`s and `Uint8Array`s spelled out. */
function show(value: unknown): string {
  return (
    JSON.stringify(
      value,
      (...[, value]) =>
        typeof value === "bigint"
          ? `${value}n`
          : value instanceof Uint8Array
            ? `Uint8Array(${value.byteLength})`
            : value,
      2,
    ) ?? String(value)
  );
}

async function run() {
  try {
    const call = new Function("viewer", "rerun", `return (async () => (${code.value}))();`);
    output.textContent = show(await call(window.viewer, window.rerun));
  } catch (err) {
    const code = err instanceof rerun.ViewerControlError ? ` (${err.code})` : "";
    output.textContent = `${err}${code}`;
  }
}

for (const name of Object.keys(SNIPPETS)) {
  snippets.add(new Option(name, name));
}
snippets.addEventListener("change", () => {
  code.value = SNIPPETS[snippets.value];
});
code.value = SNIPPETS[snippets.value];
$("run").addEventListener("click", () => {
  void run();
});

/** What to open: `?url=…`, or nothing for `?url=`. A short recording by default. */
const url = new URLSearchParams(location.search).get("url") ?? "/example.rrd";

window.rerun = rerun;
window.viewer = new rerun.WebViewer();
await window.viewer.start(url === "" ? null : new URL(url, location.href).href, $("viewer"), {
  width: "100%",
  height: "100%",
  hideWelcomeScreen: true,
});
