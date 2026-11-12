import assert from "node:assert/strict";
import { test } from "node:test";

import { parseExactJson, stringifyExactJson } from "./exact_json.ts";

test("keeps integers beyond 2^53 exact, both ways", () => {
  const text = '{"range":[1761037865000000001,-1761037865000000002]}';
  const value = parseExactJson(text);
  assert.deepEqual(value, { range: [1761037865000000001n, -1761037865000000002n] });
  assert.equal(stringifyExactJson(value), text);
});

test("leaves safe integers, floats, and exponents as numbers", () => {
  assert.deepEqual(
    parseExactJson("[9007199254740991, 1.5, 1e300, -0.25]"),
    [9007199254740991, 1.5, 1e300, -0.25],
  );
});

test("leaves digits inside strings alone, escaped quotes included", () => {
  const text = '{"a":"12345678901234567890","b":"say \\"99999999999999999999\\""}';
  assert.deepEqual(parseExactJson(text), {
    a: "12345678901234567890",
    b: 'say "99999999999999999999"',
  });
});

test("rejects what JSON.parse rejects", () => {
  assert.throws(() => parseExactJson("{"));
});

test("writes a bigint as a number even with a BigInt.prototype.toJSON polyfill", () => {
  BigInt.prototype.toJSON = function () {
    return this.toString();
  };
  try {
    assert.equal(stringifyExactJson({ t: 1761037865000000001n }), '{"t":1761037865000000001}');
  } finally {
    delete BigInt.prototype.toJSON;
  }
});
