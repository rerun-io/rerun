// @ts-check

/**
 * Return a GitHub Actions input, returning `null` if it was not set.
 *
 * @param {string} name
 * @returns {string | null}
 */
export function getInput(name) {
  // @ts-expect-error: `process` is not defined without the right type definitions
  return process.env[`INPUT_${name.replace(/ /g, "_").toUpperCase()}`] ?? null;
}

/**
 * Return a GitHub Actions input, throwing an error if it was not set.
 *
 * @param {string} name
 * @returns {string}
 */
export function getRequiredInput(name) {
  const input = getInput(name);
  if (!input) {
    throw new Error(`missing required input \`${name}\``);
  }
  return input;
}

/**
 * Assert that `value` is truthy, throwing an error if it is not.
 *
 * @param {any} value
 * @param {string} [message]
 * @returns {asserts value}
 */
export function assert(value, message) {
  if (!value) {
    throw new Error(`assertion failed` + (message ? ` ${message}` : ""));
  }
}

/**
 * Returns a function that attempts to find an object with
 * `key` set to `value` in an array of objects with `key` properties.
 *
 * @template {string} Key
 * @template {{ [p in Key]: string }} T
 * @param {Key} key
 * @param {string} value
 * @returns {(a: T[]) => T|null}
 */
export function find(key, value) {
  return (a) => a.find((v) => v[key] === value) ?? null;
}

/**
 * Returns a function that attempts to retrieve the value at `index` from an array.
 *
 * @template T
 * @param {number} index
 * @returns {(a: T[]) => T|null}
 */
export function get(index) {
  return (a) => a[index] ?? null;
}

