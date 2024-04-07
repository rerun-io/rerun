// @ts-check

import { Client } from "./vercel.mjs";
import { assert, info } from "./util.mjs";

const client = new Client("NzuZ9WBTnfUGiwHrhd7mit2E");

const teamName = "rerun";
const projectName = "landing";

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

const response = await client.deployPreviewFrom(
  team.id,
  latestProductionDeployment.uid,
  "rerun-custom-preview-test",
  {
    RELEASE_COMMIT: "main",
  },
);
console.log(response);
