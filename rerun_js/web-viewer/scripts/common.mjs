import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

/** @type {typeof execSync} */
export const $ = (...args) => execSync(...args);
export const argv = process.argv.slice(2);
export const script_dir = path.dirname(fileURLToPath(import.meta.url));
export { path };

