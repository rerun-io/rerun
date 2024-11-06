// @ts-check

import { setOutput } from "../util.mjs";
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
export async function deployToPreview(client, options) {
  const project = await client.project(options.team, options.project);
  const deployment = await project.latestProductionDeployment();

  let line = `Deploying preview`;
  if (options.commit) line += ` RELEASE_COMMIT=${options.commit}`;
  if (options.version) line += ` RELEASE_VERSION=${options.version}`;
  console.log(line);

  const env = { IS_PR_PREVIEW: "true" };
  if (options.commit) env["RELEASE_COMMIT"] = options.commit;
  if (options.version) env["RELEASE_VERSION"] = options.version;

  const newDeployment = await project.deployPreviewFrom(
    deployment.uid,
    "landing-preview",
    env,
  );
  setOutput("vercel_preview_deployment_id", newDeployment.id);
  setOutput("vercel_preview_url", newDeployment.url);
  setOutput("vercel_preview_inspector_url", newDeployment.inspectorUrl);

  return newDeployment;
}
