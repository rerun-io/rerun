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
  get_state: "await viewer.get_state()",
  set_time: 'await viewer.set_time({ sequence: 0, play_state: "paused" })',
  play: 'await viewer.set_time({ play_state: "playing" })',
  follow: 'await viewer.set_time({ play_state: "following" })',
  get_location: "await viewer.get_location()",
  set_location: "await viewer.set_location((await viewer.get_state()).recordings[0].location)",
  open: 'await viewer.open(new URL("/example.rrd", location.href).href)',
  close: 'await viewer.close("current")',
  get_logs: "await viewer.get_logs()",
  get_recording_schema: "await viewer.get_recording_schema()",
  get_dataset_schema: "await viewer.get_dataset_schema()",
  get_dataset_assets_schema: "await viewer.get_dataset_assets_schema()",
  get_table_schema: "await viewer.get_table_schema()",
  get_blueprint: 'await viewer.get_blueprint({ kind: "recording" })',
  set_blueprint:
    'await viewer.set_blueprint((await viewer.get_blueprint({ kind: "recording" })).blueprint, { kind: "recording" })',
  get_transform_graph: "await viewer.get_transform_graph()",
  take_screenshot: "(await viewer.take_screenshot({ max_size: 256 })).byteLength",
  highlight_rect: 'await viewer.highlight_rect({ x: 20, y: 20, w: 200, h: 100 }, "Look here")',
  list_commands: "(await viewer.list_commands()).map((command) => command.id)",
  run_command: "await viewer.run_command((await viewer.list_commands())[0].id)",
  override_panel_state: 'viewer.override_panel_state("blueprint", "hidden")',
  toggle_panel_overrides: "viewer.toggle_panel_overrides()",
  save_recording: "(await new Response(viewer.save_recording()).arrayBuffer()).byteLength",
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
      (_key, value) =>
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
  hide_welcome_screen: true,
});
