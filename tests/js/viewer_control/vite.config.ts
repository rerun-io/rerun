import { defineConfig } from "vite";

export default defineConfig({
  // Serves `example.rrd` and the other example assets at the root, for the tests and by hand.
  publicDir: "../../../examples/assets",

  // `@rerun-io/web-viewer` is symlinked from `rerun_js/web-viewer`, which serves its `.wasm` from there.
  server: { fs: { allow: ["../../.."] } },
});
