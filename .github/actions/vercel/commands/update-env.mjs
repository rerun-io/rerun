// @ts-check

import { Client } from "../vercel.mjs";

/**
 *
 * @param {Client} client
 * @param {{
 *   team: string;
 *   project: string;
 *   commit: string | null;
 *   version: string | null;
 * }} options
 */
export async function updateProjectEnv(client, options) {
  const project = await client.project(options.team, options.project);

  if (options.commit) await project.setEnv("RELEASE_COMMIT", options.commit);
  if (options.version) await project.setEnv("RELEASE_VERSION", options.version);
}
