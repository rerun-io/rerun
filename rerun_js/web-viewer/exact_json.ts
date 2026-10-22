/**
 * JSON with exact 64-bit integers.
 *
 * `JSON.parse` turns every number into a `number`, which rounds integers beyond 2^53,
 * such as nanosecond timestamps. These parse such integers as `bigint` instead,
 * and write a `bigint` back as a plain JSON number.
 */

/** Parse `text` as JSON, with each integer outside the safe range as a `bigint`. */
export function parse_exact_json(text: string): unknown {
  const marker = new_marker();
  let quoted = "";
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '"') {
      const end = string_end(text, i);
      quoted += text.slice(i, end);
      i = end;
    } else if (c === "-" || ("0" <= c && c <= "9")) {
      const token = /^-?\d+(\.\d+)?([eE][+-]?\d+)?/.exec(text.slice(i))?.[0] ?? c;
      const is_unsafe_integer = /^-?\d+$/.test(token) && !Number.isSafeInteger(Number(token));
      quoted += is_unsafe_integer ? `"${marker}${token}"` : token;
      i += token.length;
    } else {
      quoted += c;
      i += 1;
    }
  }
  return JSON.parse(quoted, (_key, value) =>
    typeof value === "string" && value.startsWith(marker) ? BigInt(value.slice(marker.length)) : value,
  );
}

/** Write `value` as JSON, with each `bigint` as a plain JSON number. */
export function stringify_exact_json(value: unknown): string {
  const marker = new_marker();
  // Read `this[key]` rather than `value`: `JSON.stringify` calls a `toJSON` before the replacer,
  // and a common polyfill gives `BigInt.prototype` one that turns it into a string.
  const json = JSON.stringify(value, function (this: Record<string, unknown>, key, value) {
    const raw = this[key];
    return typeof raw === "bigint" ? `${marker}${raw}` : value;
  });
  return json.replaceAll(new RegExp(`"${marker}(-?\\d+)"`, "g"), "$1");
}

/** A prefix that no string in the document starts with, so it can tag the integers. */
function new_marker(): string {
  return `rerun-exact-json-${Math.random().toString(36).slice(2)}-${Date.now()}:`;
}

/** The index just past the string literal that starts at `start`. */
function string_end(text: string, start: number): number {
  let i = start + 1;
  while (i < text.length && text[i] !== '"') {
    i += text[i] === "\\" ? 2 : 1;
  }
  return i + 1;
}
