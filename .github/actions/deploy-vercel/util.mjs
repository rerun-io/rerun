// @ts-check

/**
 * @param {string} name
 * @returns {string | null}
 */
export function getInput(name) {
  // @ts-expect-error: `process` is not defined without the right type definitions
  return process.env[`INPUT_${name.replace(/ /g, "_").toUpperCase()}`] ?? null;
}

/**
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
 *
 * @template {string} Key
 * @template {{ [p in Key]: string }} T
 * @param {Key} key
 * @param {string} name
 * @returns {(a: T[]) => T|null}
 */
export function find(key, name) {
  return (a) => a.find((v) => v[key] === name) ?? null;
}

/**
 * @template T
 * @param {number} index
 * @returns {(a: T[]) => T|null}
 */
export function get(index) {
  return (a) => a[index] ?? null;
}

