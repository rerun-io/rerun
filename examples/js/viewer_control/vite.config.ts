import { defineConfig } from "vite";

export default defineConfig({
  // `@rerun-io/web-viewer` is symlinked from `rerun_js/web-viewer`, which serves its `.wasm` from there.
  server: { fs: { allow: ["../../.."] } },
});
