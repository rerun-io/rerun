// @ts-check

import { Client } from "./vercel.mjs";
import { assert, getRequiredInput, info } from "./util.mjs";

// These inputs are defined in `action.yml`, and should be kept in sync
const token = getRequiredInput("vercel_token");
const teamName = getRequiredInput("vercel_team_name");
const projectName = getRequiredInput("vercel_project_name");
const releaseCommit = getRequiredInput("release_commit");
const releaseVersion = getRequiredInput("release_version");

const client = new Client(token);

info`Fetching team "${teamName}"`;
const availableTeams = await client.teams();
assert(availableTeams, `failed to get team "${teamName}"`);
const team = availableTeams.find((team) => team.name === teamName);
assert(team, `failed to get team "${teamName}"`);

info`Fetching project "${projectName}"`;
const projectsInTeam = await client.projects(team.id);
const project = projectsInTeam.find((project) => project.name === projectName);
assert(project, `failed to get project "${projectName}"`);

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

info`Setting "${RELEASE_COMMIT_KEY}" env to "${releaseCommit}"`;
await client.setEnv(team.id, project.id, releaseCommitEnv.id, {
  key: RELEASE_COMMIT_KEY,
  value: releaseCommit,
});

info`Setting "${RELEASE_VERSION_KEY}" env to "${releaseVersion}"`;
await client.setEnv(team.id, project.id, releaseVersionEnv.id, {
  key: RELEASE_VERSION_KEY,
  value: releaseVersion,
});

info`Triggering redeploy`;
await client.redeploy(team.id, latestProductionDeployment.uid, "landing");
