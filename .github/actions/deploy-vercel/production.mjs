// @ts-check

import { assert, info } from "./util.mjs";
import { Client } from "./vercel.mjs";

/**
 *
 * @param {Client} client
 * @param {{
 *   team: string;
 *   project: string;
 *   commit: string;
 *   version: string;
 * }} options
 */
export async function deployToProduction(client, options) {
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

  const environment = await client.envs(team.id, project.id);
  const RELEASE_COMMIT_KEY = "RELEASE_COMMIT";
  const RELEASE_VERSION_KEY = "RELEASE_VERSION";

  info`Fetching "${RELEASE_COMMIT_KEY}" env var`;
  const releaseCommitEnv = environment.find(
    (env) => env.key === RELEASE_COMMIT_KEY,
  );
  assert(releaseCommitEnv, `failed to get "${RELEASE_COMMIT_KEY}" env var`);

  info`Fetching "${RELEASE_VERSION_KEY}" env var`;
  const releaseVersionEnv = environment.find(
    (env) => env.key === RELEASE_VERSION_KEY,
  );
  assert(releaseVersionEnv, `failed to get "${RELEASE_VERSION_KEY}" env var`);

  info`Setting "${RELEASE_COMMIT_KEY}" env to "${options.commit}"`;
  await client.setEnv(team.id, project.id, releaseCommitEnv.id, {
    key: RELEASE_COMMIT_KEY,
    value: options.commit,
  });

  info`Setting "${RELEASE_VERSION_KEY}" env to "${options.version}"`;
  await client.setEnv(team.id, project.id, releaseVersionEnv.id, {
    key: RELEASE_VERSION_KEY,
    value: options.version,
  });

  info`Triggering redeploy`;
  await client.redeploy(team.id, latestProductionDeployment.uid, "landing");
}
