import assert from "node:assert/strict";
import { test } from "node:test";

import { parse_store_id } from "./store_id.ts";

test("splits a store id into its parts", () => {
  assert.deepEqual(parse_store_id("Recording:my_app:rec1"), {
    kind: "Recording",
    application_id: "my_app",
    recording_id: "rec1",
  });
});

test("unescapes the application id, and keeps colons in the recording id", () => {
  assert.deepEqual(parse_store_id("Blueprint:a\\:b\\\\c:x:y"), {
    kind: "Blueprint",
    application_id: "a:b\\c",
    recording_id: "x:y",
  });
});

test("an empty recording id", () => {
  assert.equal(parse_store_id("Recording:app:").recording_id, "");
});
