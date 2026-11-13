import { build } from "esbuild";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const { outputFiles } = await build({
  entryPoints: [fileURLToPath(new URL("./store_id.ts", import.meta.url))],
  bundle: true,
  write: false,
  format: "esm",
});
const { parseStoreId } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString("base64")}`
);

test("splits a store id into its parts", () => {
  assert.deepEqual(parseStoreId("Recording:my_app:rec1"), {
    kind: "Recording",
    applicationId: "my_app",
    recordingId: "rec1",
  });
});

test("unescapes the application id, and keeps colons in the recording id", () => {
  assert.deepEqual(parseStoreId("Blueprint:a\\:b\\\\c:x:y"), {
    kind: "Blueprint",
    applicationId: "a:b\\c",
    recordingId: "x:y",
  });
});

test("an empty recording id", () => {
  assert.equal(parseStoreId("Recording:app:").recordingId, "");
});
