/**
 * JSON with exact 64-bit integers.
 *
 * `JSON.parse` turns every number into a `number`, which rounds integers beyond 2^53,
 * such as nanosecond timestamps. These parse such integers as `bigint` instead,
 * and write a `bigint` back as a plain JSON number.
 */

/** Parse `text` as JSON, with each integer outside the safe range as a `bigint`. */
export function parseExactJson(text: string): unknown {
  const marker = newMarker();
  let quoted = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '"') {
      const end = stringEnd(text, i);
      quoted += text.slice(i, end);
      i = end;
    } else if (c === "-" || ("0" <= c && c <= "9")) {
      const token = /^-?\d+(\.\d+)?([eE][+-]?\d+)?/.exec(text.slice(i))?.[0] ?? c;
      const isUnsafeInteger = /^-?\d+$/.test(token) && !Number.isSafeInteger(Number(token));
      quoted += isUnsafeInteger ? `"${marker}${token}"` : token;
      i += token.length;
    } else {
      quoted += c;
      i += 1;
    }
  }
  return JSON.parse(quoted, (...[, value]) =>
    typeof value === "string" && value.startsWith(marker)
      ? BigInt(value.slice(marker.length))
      : value,
  );
}

/** Write `value` as JSON, with each `bigint` as a plain JSON number. */
export function stringifyExactJson(value: unknown): string {
  const marker = newMarker();
  // Read `this[key]` rather than `value`: `JSON.stringify` calls a `toJSON` before the replacer,
  // and a common polyfill gives `BigInt.prototype` one that turns it into a string.
  const json = JSON.stringify(value, function (this: Record<string, unknown>, key, value) {
    const raw = this[key];
    return typeof raw === "bigint" ? `${marker}${raw}` : value;
  });
  return json.replace(new RegExp(`"${marker}(-?\\d+)"`, "g"), "$1");
}

/** @deprecated Use {@link parseExactJson} instead. */
// oxlint-disable-next-line id-match -- Deprecated public export.
export const parse_exact_json = parseExactJson;

/** @deprecated Use {@link stringifyExactJson} instead. */
// oxlint-disable-next-line id-match -- Deprecated public export.
export const stringify_exact_json = stringifyExactJson;

/** A prefix that no string in the document starts with, so it can tag the integers. */
function newMarker(): string {
  return `rerun-exact-json-${Math.random().toString(36).slice(2)}-${Date.now()}:`;
}

/** The index just past the string literal that starts at `start`. */
function stringEnd(text: string, start: number): number {
  let i = start + 1;
  while (i < text.length && text[i] !== '"') {
    i += text[i] === "\\" ? 2 : 1;
  }
  return i + 1;
}
