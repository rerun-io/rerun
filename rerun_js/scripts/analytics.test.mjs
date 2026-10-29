import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";
import React, { createRef } from "react";

const html = readFileSync(
  new URL(
    "../../crates/top/re_web_viewer_server/web_viewer/index.html",
    import.meta.url,
  ),
  "utf8",
);
const script = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)]
  .map((match) => match[1])
  .find((source) => source.includes("function on_wasm_loaded()"));
const initialization = script.slice(
  0,
  script.indexOf("function show_center_html("),
);
const queryParser = script.slice(
  script.indexOf("function get_query_bool("),
  script.indexOf("function on_app_started("),
);
const startViewer = script.slice(
  script.indexOf("function on_wasm_loaded()"),
  script.indexOf("function get_query_bool("),
);

for (const [query, enabled] of [
  ["", true],
  ["analytics=0", false],
  ["analytics=false", false],
  ["analytics=1", true],
  ["analytics=true", true],
  ["analytics=", true],
  ["analytics=invalid", true],
  ["analytics=0&analytics=false", false],
  ["analytics=true&analytics=0", false],
  ["analytics=false&analytics=true", false],
]) {
  test(`Viewer URL ${query || "(default)"} applies the same analytics policy to both senders`, () => {
    const scripts = [];
    let options;
    const location = {
      search: `?${query}`,
      hostname: "app.rerun.io",
      origin: "https://app.rerun.io",
    };
    const context = vm.createContext({
      URLSearchParams,
      location,
      window: { location },
      console: { debug() {}, warn() {} },
      document: {
        createElement: () => ({ dataset: {} }),
        getElementById: () => ({}),
        body: { appendChild: (element) => scripts.push(element) },
      },
      setTimeout() {},
      on_wasm_error(error) {
        throw error;
      },
      wasm_bindgen: {
        WebHandle: class {
          constructor(value) {
            options = value;
          }
          has_panicked() {
            return false;
          }
          start() {
            return { then: () => ({ catch() {} }) };
          }
        },
      },
    });
    vm.runInContext(
      `${queryParser}\n${initialization}\n${startViewer}\non_wasm_loaded();`,
      context,
    );
    assert.equal(options.analytics, enabled);
    assert.equal(scripts.length, enabled ? 1 : 0);
  });
}

const reactSource = readFileSync(
  new URL("../web-viewer-react/index.js", import.meta.url),
  "utf8",
)
  .replace(/^import .*;\n/gm, "")
  .replace("export default class WebViewer", "class WebViewer");

function mountReactViewer(props) {
  const starts = [];
  const stops = [];
  class Viewer {
    ready = true;
    start(_rrd, _parent, options) {
      starts.push(options);
      return Promise.resolve();
    }
    stop() {
      stops.push(this);
    }
    on() {}
    open() {}
    close() {}
  }
  const context = vm.createContext({
    React,
    createRef,
    rerun: { WebViewer: Viewer },
  });
  vm.runInContext(
    `${reactSource}\nglobalThis.ViewerComponent = WebViewer;`,
    context,
  );
  const component = new context.ViewerComponent({ rrd: [], ...props });
  component.componentDidMount();
  return { component, starts, stops };
}

test("React applies the integrator's analytics opt-out on mount", async () => {
  const { starts } = mountReactViewer({ analytics: false });
  await Promise.resolve();
  assert.equal(starts[0].analytics, false);
});

test("React restarts with the new analytics policy when it changes", async () => {
  const { component, starts, stops } = mountReactViewer({ analytics: true });
  const previous = component.props;
  component.props = { ...previous, analytics: false };
  component.componentDidUpdate(previous);
  await Promise.resolve();
  assert.equal(stops.length, 1);
  assert.equal(starts.length, 2);
  assert.equal(starts[1].analytics, false);
});
