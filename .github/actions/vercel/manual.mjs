#!/usr/bin/env node

// Convenience script for running `index.mjs` locally,
// passing in inputs as CLI args instead of env vars.

// @ts-check

// Manually run the deployment:
//
//   # Redeploy `landing`:
//   node manual.mjs \
//     --command deploy \
//     --token VERCEL_TOKEN \
//     --team rerun \
//     --project landing \
//     --target production \
//     --commit RELEASE_COMMIT \
//     --version RELEASE_VERSION
//
//   # Deploy a preview of `landing` with a `RELEASE_COMMIT` override:
//   node manual.mjs \
//     --command deploy \
//     --token VERCEL_TOKEN \
//     --team rerun \
//     --project landing \
//     --target preview \
//     --commit RELEASE_COMMIT
//
//   # Only update env:
//   node manual.mjs \
//     --command update-env \
//     --token VERCEL_TOKEN \
//     --team rerun \
//     --project landing \
//     --commit RELEASE_COMMIT \
//     --version RELEASE_VERSION
//

import { execSync } from "node:child_process";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";

try {
  const { command, token, team, project, target, commit, version } = parseArgs({
    options: {
      command: { type: "string" },
      token: { type: "string" },
      team: { type: "string" },
      project: { type: "string" },
      target: { type: "string" },
      commit: { type: "string" },
      version: { type: "string" },
    },
    strict: true,
    allowPositionals: false,
  }).values;

  const env = { ...process.env, MANUAL_RUN: "true" };
  if (command) env["INPUT_COMMAND"] = command;
  if (token) env["INPUT_VERCEL_TOKEN"] = token;
  if (team) env["INPUT_VERCEL_TEAM_NAME"] = team;
  if (project) env["INPUT_VERCEL_PROJECT_NAME"] = project;
  if (target) env["INPUT_TARGET"] = target;
  if (commit) env["INPUT_RELEASE_COMMIT"] = commit;
  if (version) env["INPUT_RELEASE_VERSION"] = version;

  const cwd = path.dirname(fileURLToPath(import.meta.url));
  execSync("node index.mjs", { cwd, env, stdio: "inherit" });
} catch (err) {
  console.error(err.message);
  process.exit(1);
}
