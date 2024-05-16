// @ts-check

import { appendFileSync } from "fs";
import os from "os";

/**
 * Return a GitHub Actions input, returning `null` if it was not set.
 *
 * @param {string} name
 * @returns {string | null}
 */
export function getInput(name) {
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
 * Set a GitHub Actions output for other workflows steps to read.
 * @param {string} key
 * @param {string} value
 */
export function setOutput(key, value) {
  const s = `${key}=${value}${os.EOL}`;
  if (process.env["MANUAL_RUN"]) {
    console.log(s);
    return;
  }
  const outputFile = /** @type {string} */ (process.env["GITHUB_OUTPUT"]);
  appendFileSync(outputFile, s);
}

/**
 * Assert that `value` is truthy, throwing an error if it is not.
 *
 * @param {any} value
 * @param {string | (() => string)} [message]
 * @returns {asserts value}
 */
export function assert(value, message) {
  if (!value) {
    let error;
    if (typeof message === "string") {
      error = `assertion failed: ${message}`;
    } else if (typeof message === "function") {
      error = `assertion failed: ${message()}`;
    } else {
      error = `assertion failed`;
    }
    throw new Error(error);
  }
}
