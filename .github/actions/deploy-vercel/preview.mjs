// @ts-check

import { assert, info, setOutput } from "./util.mjs";
import { Client } from "./vercel.mjs";

/**
 *
 * @param {Client} client
 * @param {{
 *   team: string;
 *   project: string;
 *   commit: string;
 * }} options
 */
export async function deployToPreview(client, options) {
  info`Fetching team "${options.team}"`;
  const availableTeams = await client.teams();
  assert(availableTeams, `failed to get team "${options.team}"`);
  const team = availableTeams.find((team) => team.name === options.team);
  assert(team, `failed to get team "${options.team}"`);

  info`Fetching project "${options.project}"`;
  const projectsInTeam = await client.projects(team.id);
  const project = projectsInTeam.find(
    (project) => project.name === options.project,
  );
  assert(project, `failed to get project "${options.project}"`);

  info`Fetching latest production deployment`;
  const productionDeployments = await client.deployments(team.id, project.id);
  const latestProductionDeployment = productionDeployments[0];
  assert(
    latestProductionDeployment,
    `failed to get latest production deployment`,
  );

  info`Deploying preview with RELEASE_COMMIT=${options.commit}`;
  const { url } = await client.deployPreviewFrom(
    team.id,
    latestProductionDeployment.uid,
    "landing-preview",
    {
      RELEASE_COMMIT: options.commit,
      IS_PR_PREVIEW: "true",
    },
  );

  setOutput("vercel_preview_url", url);
}
