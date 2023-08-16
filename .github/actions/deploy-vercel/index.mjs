// @ts-check

import { Client } from "./client.mjs";
import { assert, find, get, getRequiredInput } from "./util.mjs";

const token = getRequiredInput("vercel_token");
const teamName = getRequiredInput("vercel_team_name");
const projectName = getRequiredInput("vercel_project_name");
const releaseCommit = getRequiredInput("release_commit");

const client = new Client(token);

console.log(`Fetching team \`${teamName}\``);
const team = await client.teams().then(find("name", teamName));
assert(team, `failed to get team \`${teamName}\``);

console.log(`Fetching project \`${projectName}\``);
const project = await client.projects(team.id).then(find("name", projectName));
assert(project, `failed to get project \`${projectName}\``);

console.log(`Fetching latest deployment`);
const deployment = await client.deployments(team.id, project.id).then(get(0));
assert(deployment, `failed to get latest deployment`);

console.log(`Fetching \`RELEASE_COMMIT\` env var`);
const env = await client.envs(team.id, project.id).then(find("key", "RELEASE_COMMIT"));
assert(env, `failed to get \`RELEASE_COMMIT\` env var`);

console.log(`Setting \`RELEASE_COMMIT\` env to \`${releaseCommit}\``);
await client.setEnv(team.id, project.id, env.id, { key: "RELEASE_COMMIT", value: releaseCommit });

console.log(`Triggering redeploy`);
await client.redeploy(team.id, deployment.uid, "landing");

