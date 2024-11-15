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
export async function deployToProduction(client, options) {
  const project = await client.project(options.team, options.project);
  const deployment = await project.latestProductionDeployment();

  if (options.commit) await project.setEnv("RELEASE_COMMIT", options.commit);
  if (options.version) await project.setEnv("RELEASE_VERSION", options.version);

  const newDeployment = await project.redeploy(deployment.uid, "landing");

  const result = await project.waitForDeployment(newDeployment.id);
  if (result.type === "failure") {
    throw new Error(
      `deployment failed, see ${JSON.stringify(
        result.deployment.inspectorUrl,
      )} for more information`,
    );
  }
}
