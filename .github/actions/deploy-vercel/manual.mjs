#!/usr/bin/env node

// Manually run the deployment:
//
//   node manual.mjs \
//     --token VERCEL_TOKEN \
//     --team rerun \
//     --project landing \
//     --commit RELEASE_COMMIT
//

import { execSync } from "node:child_process";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { assert } from "./util.mjs";

const dirname = path.dirname(fileURLToPath(import.meta.url));

/** @type {typeof execSync} */
const $ = (cmd, opts) => execSync(cmd, { stdio: "inherit", ...opts });

const { token, team, project, commit } = parseArgs({
  options: {
    token: { type: "string" },
    team: { type: "string" },
    project: { type: "string" },
    commit: { type: "string" },
  },
  strict: true,
  allowPositionals: false,
}).values;
assert(token, "missing `--token`");
assert(team, "missing `--team`");
assert(project, "missing `--project`");
assert(commit, "missing `--commit`");

$("node index.mjs", {
  cwd: dirname,
  env: {
    ...process.env,
    INPUT_VERCEL_TOKEN: token,
    INPUT_VERCEL_TEAM_NAME: team,
    INPUT_VERCEL_PROJECT_NAME: project,
    INPUT_RELEASE_COMMIT: commit,
  },
});

